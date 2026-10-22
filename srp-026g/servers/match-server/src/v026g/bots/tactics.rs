//! Where to stand and how to get there: the `tactics` layer
//! (`player_bots_research.md` B1, B6). Plays ask; they never walk the grid
//! themselves.
//!
//! - **Routes** keep off the floors a body must not be on: a damage zone
//!   while it is active (`hazards`), and a spawn area (the owner's rule:
//!   never into the enemy's; never back into one's own once out of it).
//! - **Spots** ([`SpotQuery`]): floors at a distance band from a point,
//!   with a line of sight to it or hidden from another, higher or lower,
//!   off the hazards and spawn areas. Cheap filters first, rays last
//!   (research B5).
//! - **Exits** of a side's base: points 30-60 m out on the ways from its
//!   spawns toward the other side (play 15).

use std::sync::Arc;

use mapdata::regions::Regions;
use mapdata::{NodeId, Shape, Vec3, World};

use super::hazards::{HazardMarks, Hazards, PASS_MS};
use super::path::{self, Route};
use super::rng::Rng;
use super::view::{EYE_CROUCHED, EYE_STANDING};

/// A spawn area: this far round a side's spawn points, metres (GUESS from
/// the maps: spawn points of a side lie in a few clusters 10-20 m across).
pub const SPAWN_AREA_M: f32 = 20.0;
/// A point is looked for on the grid within this, metres.
const LOCATE_M: f32 = 3.0;
/// A use reaches this far from the eye, metres: the client's ray is
/// `usable_objects_detection_distance`, 2.0 (PROVEN), and must hit the
/// object's own shape; 0.2 is kept for the aim and the body's sway.
pub const USE_REACH_M: f32 = 1.8;
/// A way out of a zone is looked for over at most this many floors.
const ESCAPE_FLOORS: usize = 20_000;
/// A way out ends at least this far from where it starts, metres: past the
/// motor's arrival distance, so the body really walks out.
const ESCAPE_MIN_M: f32 = 1.5;
/// A sight ray's first hit this near a body is on the body, metres: on the
/// real maps the rays toward a container's use box stop on its faces (0.00
/// from the box, 2026-10-16 bench); this is for rounding.
const BODY_HIT_M: f32 = 0.1;

/// What one map's tactics hold for one hazard margin: the same for every
/// match on the map, so matches share it ([`Tactics::of`]). Read-only once
/// built; a match's own state (the anomalies' clocks, the human routes)
/// is in [`Tactics`].
pub struct TacticsBase {
    pub world: Arc<World>,
    /// The zones and the floors they mark, every zone counting as active
    /// (no anomalies attached): each match clones it and attaches its own.
    hazards: Hazards,
    /// The bases' exits by map side, worked out once.
    exits: Vec<(i32, Vec<Vec3>)>,
    /// Per floor, a bit per side (`1 << side`, sides 0-7) whose spawn area
    /// it lies in ([`Tactics::allowed`]).
    spawn_areas: Vec<u8>,
    /// The grid's regions cut by the floors' labels (the zones' set and the
    /// spawn areas), so [`Tactics::allowed`] is the same on every floor of
    /// a region (`mapdata::regions`).
    regions: Regions,
    /// Per floor, a bit: a walk from a spawn gets there (any link, every
    /// zone off). A goal on any other floor (a roof one only drops from, a
    /// shelf's top past the jump) has no way from where bots are.
    walkable: Vec<u64>,
}

/// The past-fixed-zones regions and the zones they were made for
/// ([`Tactics::past_fixed_zones`]).
type PastFixed = (Vec<bool>, Arc<Vec<u64>>);

/// One map's tactics for a match: the shared [`TacticsBase`] and the
/// match's own hazard clocks and human routes.
pub struct Tactics {
    base: Arc<TacticsBase>,
    pub world: Arc<World>,
    pub hazards: Hazards,
    /// The routes humans took on this map and mode (play 17), on its
    /// floors; empty without `SRP_BOT_ROUTES`.
    pub routes: Vec<super::routes::HumanRoute>,
    /// [`Self::past_fixed_zones`]'s regions, and the zones it was made for
    /// ([`Hazards::fixed_zones`]): made again only when they change (at
    /// the anomalies' attach).
    past_fixed: std::sync::Mutex<Option<PastFixed>>,
}

/// The label's bits for the sides' spawn areas.
const SPAWN_BITS: u32 = 8;

/// The version of what [`TacticsBase::build`] gives, in its key: raised
/// with any change that changes the tactics of a map (the hazard marks,
/// the spawn areas, the regions, the exits and the searches they come
/// from), so a stored base of the old build is never read for the new.
/// `the_base_and_its_digest_are_fixed` fails until then.
pub const TACTICS_ALGO: u32 = 2;

/// What a spot is for.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SpotQuery {
    pub around: Vec3,
    pub min_m: f32,
    pub max_m: f32,
    /// The spot must see this point (from crouched eyes).
    pub sight_to: Option<Vec3>,
    /// The solid `sight_to` lies in (a container's box): a sight line that
    /// ends on it sees it ([`Tactics::sees_body`]).
    pub sight_body: Option<Shape>,
    /// The spot must not be seen from this point.
    pub hidden_from: Option<Vec3>,
    /// Prefer spots higher than `around` (by this weight per metre).
    pub prefer_high: f32,
    /// The bot's map side: the other side's spawn area is out.
    pub side: Option<i32>,
    /// Candidates sampled before the rays.
    pub samples: usize,
}

impl SpotQuery {
    pub fn ring(around: Vec3, min_m: f32, max_m: f32) -> Self {
        Self {
            around,
            min_m,
            max_m,
            sight_to: None,
            sight_body: None,
            hidden_from: None,
            prefer_high: 0.0,
            side: None,
            samples: 48,
        }
    }
}

/// A spot found, with its score (higher is better).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Spot {
    pub at: Vec3,
    pub node: NodeId,
    pub score: f32,
}

impl TacticsBase {
    /// Builds them: the hazard marks, the floors' labels, the regions, the
    /// bases' exits. 0.15-0.3 s on the big maps on a fast machine, so off
    /// the match loop.
    pub fn build(world: Arc<World>, hazard_margin: f32) -> Self {
        let hazards = Hazards::build(&world, hazard_margin);
        let grid = &world.grid;
        // The zones' set in the high bits, the sides' spawn areas in the low.
        let mut labels: Vec<u32> = (0..grid.len() as NodeId)
            .map(|node| u32::from(hazards.set_of(node)) << SPAWN_BITS)
            .collect();
        for spawn in &world.map.spawns {
            let bit = 1u32 << (spawn.team.clamp(0, SPAWN_BITS as i32 - 1));
            let p = spawn.position;
            let area = [
                p.x - SPAWN_AREA_M,
                p.z - SPAWN_AREA_M,
                p.x + SPAWN_AREA_M,
                p.z + SPAWN_AREA_M,
            ];
            for node in grid.floors_in(area) {
                if grid.position(node).distance_xz(p) < SPAWN_AREA_M {
                    labels[node as usize] |= bit;
                }
            }
        }
        let regions = Regions::build(grid, Some(&labels));
        // Only the spawn areas are read from here on: the zones' set is
        // the hazards' own.
        let spawn_areas = labels.iter().map(|&label| label as u8).collect();
        drop(labels);
        let walkable = walkable_from_spawns(&world);
        let mut base = Self {
            world,
            hazards,
            exits: Vec::new(),
            spawn_areas,
            regions,
            walkable,
        };
        let sides: Vec<i32> = {
            let mut sides: Vec<i32> = base.world.map.spawns.iter().map(|s| s.team).collect();
            sides.sort_unstable();
            sides.dedup();
            sides
        };
        for side in sides {
            let exits = base.find_exits(side);
            base.exits.push((side, exits));
        }
        base
    }

    /// The hazard margin it was built for.
    pub fn margin(&self) -> f32 {
        self.hazards.margin
    }

    /// The key of the base built for `margin` from the map file whose
    /// bytes digest to `map_file` (`mapdata::digest::Digest::bytes`: the
    /// map's zones, anomalies and spawns) on the grid of `grid_key`
    /// (`mapdata::grid_key`, of the triangles as this machine placed them:
    /// a base is never read onto another grid).
    pub fn key(map_file: u64, grid_key: u64, margin: f32) -> u64 {
        let mut digest = mapdata::digest::Digest::new();
        digest.u32(TACTICS_ALGO);
        digest.u64(map_file);
        digest.u64(grid_key);
        digest.f32(margin);
        digest.f32(SPAWN_AREA_M);
        digest.u32(SPAWN_BITS);
        digest.u32(PASS_MS);
        digest.finish()
    }

    /// Writes the base to `path` as the base of `key` (`mapdata::built`).
    pub fn write(&self, path: &std::path::Path, key: u64) -> Result<(), mapdata::Error> {
        mapdata::built::write(
            path,
            mapdata::built::TACTICS_MAGIC,
            key,
            self.digest(),
            |out| {
                self.hazards.marks().write_body(out)?;
                out.plain(&self.spawn_areas, |&bits| [bits])?;
                self.regions.write_body(out)?;
                out.plain(&self.walkable, |&bits| bits.to_le_bytes())?;
                out.plain(&self.exits, |(side, exits)| {
                    let mut bytes = [0u8; 8];
                    bytes[..4].copy_from_slice(&side.to_le_bytes());
                    bytes[4..].copy_from_slice(&(exits.len() as u32).to_le_bytes());
                    bytes
                })?;
                let points: Vec<Vec3> = self.exits.iter().flat_map(|(_, e)| e).copied().collect();
                out.plain(&points, |p| {
                    let mut bytes = [0u8; 12];
                    for (chunk, v) in bytes.as_chunks_mut::<4>().0.iter_mut().zip([p.x, p.y, p.z]) {
                        *chunk = v.to_bits().to_le_bytes();
                    }
                    bytes
                })
            },
        )
    }

    /// Reads the base of `key` on `world` (built for `margin`) that
    /// [`Self::write`] wrote: the zones are the world's own, the rest is
    /// read, checked index by index and then by the digest.
    pub fn read(
        path: &std::path::Path,
        key: u64,
        world: Arc<World>,
        margin: f32,
    ) -> Result<Self, mapdata::Error> {
        let zones = Hazards::zones_of(&world);
        let floors = world.grid.len();
        mapdata::built::read(
            path,
            mapdata::built::TACTICS_MAGIC,
            key,
            |input| {
                let marks = HazardMarks::read_body(input, floors, zones.len())?;
                let spawn_areas: Vec<u8> = input.plain(|[bits]: [u8; 1]| bits)?;
                let regions = Regions::read_body(input, floors)?;
                let walkable: Vec<u64> = input.plain(u64::from_le_bytes)?;
                let sides: Vec<(i32, u32)> = input.plain(|bytes: [u8; 8]| {
                    let (side, count) = bytes.split_at(4);
                    (
                        i32::from_le_bytes(side.try_into().expect("4 bytes")),
                        u32::from_le_bytes(count.try_into().expect("4 bytes")),
                    )
                })?;
                let points: Vec<Vec3> = input.plain(|bytes: [u8; 12]| {
                    let v = bytes
                        .as_chunks::<4>()
                        .0
                        .iter()
                        .map(|chunk| f32::from_bits(u32::from_le_bytes(*chunk)))
                        .collect::<Vec<_>>();
                    Vec3::new(v[0], v[1], v[2])
                })?;
                let total: u64 = sides.iter().map(|&(_, count)| u64::from(count)).sum();
                if spawn_areas.len() != floors
                    || walkable.len() != floors.div_ceil(64)
                    || total != points.len() as u64
                {
                    return Err("tactics out of range".into());
                }
                let mut rest = points.as_slice();
                let exits = sides
                    .into_iter()
                    .map(|(side, count)| {
                        let (exits, after) = rest.split_at(count as usize);
                        rest = after;
                        (side, exits.to_vec())
                    })
                    .collect();
                Ok(Self {
                    world,
                    hazards: Hazards::with_marks(zones, Arc::new(marks), margin),
                    exits,
                    spawn_areas,
                    regions,
                    walkable,
                })
            },
            Self::digest,
        )
    }

    /// Bytes held, beyond the world.
    pub fn memory_bytes(&self) -> usize {
        self.hazards.marks().memory_bytes()
            + self.spawn_areas.len()
            + self.regions.memory_bytes()
            + self.walkable.len() * 8
            + self
                .exits
                .iter()
                .map(|(_, exits)| exits.len() * 12)
                .sum::<usize>()
    }

    /// What the tactics answer, as one number (`mapdata::digest`): the
    /// zones, their marks, the spawn areas, the regions, the walkable
    /// floors and the exits.
    /// Equal digests, equal answers.
    pub fn digest(&self) -> u64 {
        let mut digest = mapdata::digest::Digest::new();
        digest.f32(self.hazards.margin);
        digest.u32(self.hazards.pass_ms);
        digest.len(self.hazards.zones.len());
        for zone in &self.hazards.zones {
            digest.len(zone.zone);
            for v in [zone.centre.x, zone.centre.y, zone.centre.z] {
                digest.f32(v);
            }
        }
        self.hazards.marks().digest_into(&mut digest);
        digest.len(self.spawn_areas.len());
        self.spawn_areas
            .iter()
            .for_each(|&bits| digest.u32(u32::from(bits)));
        self.regions.digest_into(&mut digest);
        digest.len(self.walkable.len());
        self.walkable.iter().for_each(|&bits| digest.u64(bits));
        digest.len(self.exits.len());
        for (side, exits) in &self.exits {
            digest.u32(*side as u32);
            digest.len(exits.len());
            for exit in exits {
                for v in [exit.x, exit.y, exit.z] {
                    digest.f32(v);
                }
            }
        }
        digest.finish()
    }

    /// [`Tactics::exits`].
    pub fn exits(&self, side: i32) -> &[Vec3] {
        self.exits
            .iter()
            .find(|(s, _)| *s == side)
            .map_or(&[], |(_, exits)| exits.as_slice())
    }

    /// [`Tactics::walkable`].
    fn walkable(&self, node: NodeId) -> bool {
        self.walkable
            .get(node as usize / 64)
            .is_some_and(|bits| bits >> (node % 64) & 1 == 1)
    }

    /// [`Tactics::in_spawn_area`].
    fn in_spawn_area(&self, point: Vec3, side: Option<i32>) -> bool {
        self.world
            .map
            .spawns
            .iter()
            .filter(|spawn| side.is_none_or(|side| spawn.team == side))
            .any(|spawn| spawn.position.distance_xz(point) < SPAWN_AREA_M)
    }

    fn find_exits(&self, side: i32) -> Vec<Vec3> {
        let map = &self.world.map;
        let ours: Vec<Vec3> = map
            .spawns
            .iter()
            .filter(|s| s.team == side)
            .map(|s| s.position)
            .collect();
        let theirs: Vec<Vec3> = map
            .spawns
            .iter()
            .filter(|s| s.team != side)
            .map(|s| s.position)
            .collect();
        let Some(target) = centroid(&theirs) else {
            return Vec::new();
        };
        let mut exits: Vec<Vec3> = Vec::new();
        for spawn in ours.iter().step_by(ours.len().div_ceil(6).max(1)) {
            // The whole grid's search, the very cheapest way (once a map).
            let grid = &self.world.grid;
            let Some(route) = grid
                .locate(*spawn, 1.5)
                .zip(grid.locate(target, LOCATE_M))
                .and_then(|(from, to)| {
                    // Every zone counts as active here: no anomalies are
                    // attached to the base.
                    path::plan_where(grid, from, to, |node| self.hazards.allowed(node, 0))
                })
            else {
                continue;
            };
            // The first point 30-60 m along that is out of every spawn area.
            let mut s = 30.0;
            while s <= 60.0_f32.min(route.length()) {
                let point = route.point_at(s);
                if !self.in_spawn_area(point, None) {
                    if exits.iter().all(|e| e.distance_xz(point) > 10.0) {
                        exits.push(point);
                    }
                    break;
                }
                s += 5.0;
            }
        }
        exits
    }
}

impl Tactics {
    /// A match's tactics, built for it alone ([`TacticsBase::build`]).
    pub fn new(world: Arc<World>, hazard_margin: f32) -> Self {
        Self::of(Arc::new(TacticsBase::build(world, hazard_margin)))
    }

    /// A match's tactics on a shared base: its own copy of the zones (the
    /// marks stay shared), no routes yet.
    pub fn of(base: Arc<TacticsBase>) -> Self {
        Self {
            world: base.world.clone(),
            hazards: base.hazards.clone(),
            routes: Vec::new(),
            past_fixed: Default::default(),
            base,
        }
    }

    pub fn base(&self) -> &Arc<TacticsBase> {
        &self.base
    }

    /// The human routes of this map and mode, put on its floors; a route
    /// left with no point is dropped.
    pub fn set_routes(&mut self, routes: Vec<super::routes::HumanRoute>) {
        self.routes = routes
            .into_iter()
            .map(|route| route.on_floors(&self.world.grid))
            .filter(|route| !route.points.is_empty())
            .collect();
    }

    /// The human routes out of `side`'s base.
    pub fn routes_of(&self, side: i32) -> impl Iterator<Item = &super::routes::HumanRoute> {
        self.routes.iter().filter(move |route| route.team == side)
    }

    /// Whether a walk from a spawn gets to `node` (any link, every zone
    /// off): bots are only ever on such floors, so a goal elsewhere has no
    /// way.
    pub fn walkable(&self, node: NodeId) -> bool {
        self.base.walkable(node)
    }

    /// Whether a walk from a spawn gets to `node` over floors no zone that
    /// is on all match marks ([`Hazards::fixed_at`]; the zones that switch
    /// count as off): no route to any other floor is ever found this
    /// match, whatever the clock (a battery in a pocket ringed by toxic
    /// ground: level_01's, level_03's). Walked on the regions (their
    /// labels hold the zones, so a region is shut whole or not at all, and
    /// a way between floors is one between their regions,
    /// `mapdata::regions`): a few thousand, well under a millisecond, made
    /// for the zones' controls and again only if they change.
    pub fn past_fixed_zones(&self, node: NodeId) -> bool {
        let region = self.base.regions.of(node) as usize;
        self.past_fixed_regions()
            .get(region / 64)
            .is_some_and(|bits| bits >> (region % 64) & 1 == 1)
    }

    fn past_fixed_regions(&self) -> Arc<Vec<u64>> {
        let mut cache = self
            .past_fixed
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some((made_for, bits)) = cache.as_ref()
            && made_for.len() == self.hazards.zones.len()
            && made_for
                .iter()
                .zip(&self.hazards.zones)
                .all(|(&fixed, zone)| fixed != zone.switches())
        {
            return bits.clone();
        }
        let fixed = self.hazards.fixed_zones();
        let regions = &self.base.regions;
        let grid = &self.world.grid;
        let mut bits = vec![0u64; regions.len().div_ceil(64)];
        let mut queue: Vec<u32> = Vec::new();
        let mark = |bits: &mut Vec<u64>, queue: &mut Vec<u32>, r: u32| {
            let (word, bit) = (r as usize / 64, r % 64);
            if bits[word] >> bit & 1 == 0 {
                bits[word] |= 1 << bit;
                queue.push(r);
            }
        };
        for spawn in &self.world.map.spawns {
            if let Some(node) = grid.locate(spawn.position, 1.5) {
                mark(&mut bits, &mut queue, regions.of(node));
            }
        }
        if bits.iter().all(|&w| w == 0) {
            bits.iter_mut().for_each(|w| *w = u64::MAX);
        }
        while let Some(a) = queue.pop() {
            for &(b, _) in regions.out(a) {
                if !self.hazards.fixed_at(regions.middle(b)) {
                    mark(&mut bits, &mut queue, b);
                }
            }
        }
        let bits = Arc::new(bits);
        *cache = Some((fixed, bits.clone()));
        bits
    }

    /// Whether `point` is in a spawn area of `side` (any side if `None`).
    pub fn in_spawn_area(&self, point: Vec3, side: Option<i32>) -> bool {
        self.base.in_spawn_area(point, side)
    }

    /// The map side whose spawn is nearest `point`.
    pub fn side_at(&self, point: Vec3) -> Option<i32> {
        self.world
            .map
            .spawns
            .iter()
            .min_by(|a, b| {
                a.position
                    .distance_xz(point)
                    .total_cmp(&b.position.distance_xz(point))
            })
            .map(|spawn| spawn.team)
    }

    /// Whether a body of map side `side` may walk on `node` at `time`: no
    /// active hazard, and not the other side's spawn area unless the goal
    /// is in their base (`objective`: their container, which lies 6-23 m
    /// from their spawns on the shipped maps). Its own spawn area is open:
    /// its container is there.
    pub fn allowed(&self, node: NodeId, time: u32, side: Option<i32>, objective: bool) -> bool {
        if !self.hazards.allowed(node, time) {
            return false;
        }
        let Some(side) = side.filter(|_| !objective) else {
            return true;
        };
        // No spawn area of another side.
        let own = 1u32 << side.clamp(0, SPAWN_BITS as i32 - 1);
        let spawn_areas = self
            .base
            .spawn_areas
            .get(node as usize)
            .map_or(0, |&bits| u32::from(bits));
        spawn_areas & !own == 0
    }

    /// [`Self::allowed`] for a whole region (its floors share their label).
    pub fn region_allowed(
        &self,
        region: u32,
        time: u32,
        side: Option<i32>,
        objective: bool,
    ) -> bool {
        self.allowed(self.base.regions.middle(region), time, side, objective)
    }

    /// The regions the routes go by.
    pub fn regions(&self) -> &Regions {
        &self.base.regions
    }

    /// The route from `from` to `to` for a body of map side `side`, off
    /// their spawn area.
    pub fn route(&self, from: Vec3, to: Vec3, time: u32, side: Option<i32>) -> Option<Route> {
        self.route_for(from, to, time, side, false)
    }

    /// [`Self::route`], into their base if `objective`.
    pub fn route_for(
        &self,
        from: Vec3,
        to: Vec3,
        time: u32,
        side: Option<i32>,
        objective: bool,
    ) -> Option<Route> {
        let grid = &self.world.grid;
        let start = grid.locate(from, 1.5)?;
        let goal = grid.locate(to, LOCATE_M)?;
        self.route_nodes(start, goal, time, side, objective, |_| true)
    }

    /// [`Self::route_for`] between floors, keeping off the floors `avoid`
    /// shuts too (the spots a bot got stuck at).
    pub fn route_nodes(
        &self,
        start: NodeId,
        goal: NodeId,
        time: u32,
        side: Option<i32>,
        objective: bool,
        open: impl Fn(NodeId) -> bool,
    ) -> Option<Route> {
        let mut iters = usize::MAX;
        match self.start_route(start, goal, time, side, objective, open, &mut iters) {
            Ok(route) => route,
            Err(_) => unreachable!("an unbounded search ends"),
        }
    }

    /// [`Self::route_nodes`] for at most `iters` floors expanded (taken off
    /// it, [`path::plan_start`]): the route, or none, once the search ends
    /// within them; else the search, to go on with in
    /// [`Self::resume_route`] with the same arguments.
    #[allow(clippy::too_many_arguments)]
    // The Err is the search under way (`path::plan_start`).
    #[allow(clippy::result_large_err)]
    pub fn start_route(
        &self,
        start: NodeId,
        goal: NodeId,
        time: u32,
        side: Option<i32>,
        objective: bool,
        open: impl Fn(NodeId) -> bool,
        iters: &mut usize,
    ) -> Result<Option<Route>, path::Planning> {
        path::plan_start(
            &self.world.grid,
            &self.base.regions,
            start,
            goal,
            |node| self.allowed(node, time, side, objective) && open(node),
            |region| self.region_allowed(region, time, side, objective),
            iters,
        )
    }

    /// The way so far of a [`Self::start_route`] search under way
    /// ([`path::Planning::partial`]), with the arguments it started with.
    pub fn partial_route(
        &self,
        planning: &path::Planning,
        time: u32,
        side: Option<i32>,
        objective: bool,
        open: impl Fn(NodeId) -> bool,
    ) -> Option<Route> {
        planning.partial(&self.world.grid, |node| {
            self.allowed(node, time, side, objective) && open(node)
        })
    }

    /// A [`Self::start_route`] search goes on: the route, or none, once it
    /// ends; `None` while it goes on. The floors let through must be those
    /// it started with: the same arguments, and the hazards as they were
    /// ([`Hazards::version`]).
    pub fn resume_route(
        &self,
        planning: &mut path::Planning,
        time: u32,
        side: Option<i32>,
        objective: bool,
        open: impl Fn(NodeId) -> bool,
        iters: &mut usize,
    ) -> Option<Option<Route>> {
        planning.resume(
            &self.world.grid,
            &self.base.regions,
            |node| self.allowed(node, time, side, objective) && open(node),
            |region| self.region_allowed(region, time, side, objective),
            iters,
        )
    }

    /// The way out for a body on `from`, a floor it may not walk on now
    /// (inside a zone it was pushed or stepped into): to the nearest floor
    /// at least [`ESCAPE_MIN_M`] off that it may walk on and every floor
    /// next to it too (clear of the zone's edge), over any floors, so the
    /// stretch in the zone is short. No route to any goal starts there, so
    /// without it the body would stand in the fire.
    pub fn escape(
        &self,
        from: NodeId,
        time: u32,
        side: Option<i32>,
        objective: bool,
    ) -> Option<Route> {
        let grid = &self.world.grid;
        let mut seen = std::collections::HashSet::from([from]);
        let mut queue = std::collections::VecDeque::from([from]);
        let start = grid.position(from);
        let ok = |n: NodeId| self.allowed(n, time, side, objective);
        while let Some(a) = queue.pop_front() {
            if grid.position(a).distance_xz(start) >= ESCAPE_MIN_M
                && ok(a)
                && grid.links(a).all(|(b, _)| ok(b))
            {
                return path::plan(grid, from, a);
            }
            if seen.len() > ESCAPE_FLOORS {
                return None;
            }
            for (b, _) in grid.links(a) {
                if seen.insert(b) {
                    queue.push_back(b);
                }
            }
        }
        None
    }

    /// Whether a body walks from `a` to `b` in a line, on allowed floors.
    pub fn clear_line(&self, a: Vec3, b: Vec3, time: u32, side: Option<i32>) -> bool {
        self.clear_line_for(a, b, time, side, false)
    }

    /// [`Self::clear_line`], into their base if `objective`.
    pub fn clear_line_for(
        &self,
        a: Vec3,
        b: Vec3,
        time: u32,
        side: Option<i32>,
        objective: bool,
    ) -> bool {
        let grid = &self.world.grid;
        let (Some(from), Some(to)) = (grid.locate(a, 1.0), grid.locate(b, 0.6)) else {
            return false;
        };
        path::straight_where(grid, from, to, |node| {
            self.allowed(node, time, side, objective)
        })
    }

    /// Whether `point` is a place a goal may be: on a floor a walk gets to,
    /// off hazards and the other side's spawn area.
    pub fn fine_goal(&self, point: Vec3, time: u32, side: Option<i32>) -> bool {
        self.world
            .grid
            .locate(point, LOCATE_M)
            .is_some_and(|node| self.walkable(node) && self.allowed(node, time, side, false))
    }

    /// Where a body stands to take a battery lying at `at`: `at` itself
    /// when the floor there is one a walk gets to (nearly every battery);
    /// else the walkable floor whose crouched eye is nearest it, within a
    /// use's reach ([`USE_REACH_M`]) and in a clear line (a battery on a
    /// shelf, taken from the floor beside it, as players do); none if no
    /// floor is so.
    pub fn stand_to_take(&self, at: Vec3) -> Option<Vec3> {
        let grid = &self.world.grid;
        if grid
            .locate(at, LOCATE_M)
            .is_some_and(|node| self.walkable(node))
        {
            return Some(at);
        }
        let r = USE_REACH_M;
        let mut best: Option<(f32, Vec3)> = None;
        for node in grid.floors_in([at.x - r, at.z - r, at.x + r, at.z + r]) {
            if !self.walkable(node) {
                continue;
            }
            let feet = grid.position(node);
            let eye = feet + Vec3::new(0.0, EYE_CROUCHED, 0.0);
            let d = (at - eye).length();
            if d <= r && best.is_none_or(|(b, _)| d < b) && self.sees(eye, at) {
                best = Some((d, feet));
            }
        }
        best.map(|(_, feet)| feet)
    }

    /// Whether `b` is seen from `a` (points at the eyes).
    pub fn sees(&self, a: Vec3, b: Vec3) -> bool {
        self.world.sight.visible(a, b)
    }

    /// Whether `b`, a point inside `body` (a solid of the sight tree, such
    /// as a container's box), shows from `a`: the ray toward it hits
    /// nothing, or first hits `body` itself. [`Self::sees`] never sees a
    /// point inside a solid: the box's own faces block the ray.
    pub fn sees_body(&self, a: Vec3, b: Vec3, body: &Shape) -> bool {
        let d = b - a;
        let length = d.length();
        if !length.is_finite() {
            return false;
        }
        if length <= 0.02 {
            return true;
        }
        let dir = d * (1.0 / length);
        self.world
            .sight
            .ray_cast(a, dir, length - 0.01)
            .is_none_or(|t| body.distance(a + dir * t) <= BODY_HIT_M)
    }

    /// The use box of the map's container used at `at` (its
    /// [`mapdata::Container::use_point`], as the view has it), if the map
    /// has one: the container's solid body for [`Self::sees_body`].
    pub fn container_body(&self, at: Vec3) -> Option<Shape> {
        self.world
            .map
            .containers
            .iter()
            .find(|c| (c.use_point() - at).length() < 0.5)
            .and_then(|c| c.use_shapes.first().copied())
    }

    /// The best spots for `query`, best first, at most `n`.
    pub fn spots(&self, query: &SpotQuery, time: u32, n: usize, rng: &mut Rng) -> Vec<Spot> {
        let grid = &self.world.grid;
        let mut found: Vec<Spot> = Vec::new();
        for _ in 0..query.samples {
            let angle = rng.range(0.0, std::f32::consts::TAU);
            let r = rng.range(query.min_m, query.max_m);
            let probe = Vec3::new(
                query.around.x + r * angle.cos(),
                query.around.y,
                query.around.z + r * angle.sin(),
            );
            let Some(node) = grid.locate(Vec3::new(probe.x, probe.y + 1.0, probe.z), 2.0) else {
                continue;
            };
            let at = grid.position(node);
            let d = at.distance_xz(query.around);
            if d < query.min_m
                || d > query.max_m
                || !self.walkable(node)
                || !self.allowed(node, time, query.side, false)
            {
                continue;
            }
            if found.iter().any(|spot| spot.node == node) {
                continue;
            }
            let score = query.prefer_high * (at.y - query.around.y) - d * 0.01;
            found.push(Spot { at, node, score });
        }
        found.sort_by(|a, b| b.score.total_cmp(&a.score));
        // Rays lazily, from the best down.
        let mut out = Vec::new();
        for spot in found {
            if out.len() >= n {
                break;
            }
            let eye = spot.at + Vec3::new(0.0, EYE_CROUCHED, 0.0);
            if let Some(target) = query.sight_to {
                let target = target + Vec3::new(0.0, 1.0, 0.0);
                let seen = match &query.sight_body {
                    Some(body) => self.sees_body(eye, target, body),
                    None => self.sees(eye, target),
                };
                if !seen {
                    continue;
                }
            }
            if let Some(from) = query.hidden_from
                && self.sees(
                    from + Vec3::new(0.0, EYE_STANDING, 0.0),
                    spot.at + Vec3::new(0.0, EYE_STANDING, 0.0),
                )
            {
                continue;
            }
            out.push(spot);
        }
        out
    }

    /// The exits of `side`'s base: points 30-60 m out on the ways from its
    /// spawns toward the other side's, outside the spawn areas.
    pub fn exits(&self, side: i32) -> &[Vec3] {
        self.base.exits(side)
    }
}

/// The floors a walk from a spawn's floor gets to, a bit each; every
/// floor when no spawn is on a floor (a map without spawns).
fn walkable_from_spawns(world: &World) -> Vec<u64> {
    let grid = &world.grid;
    let mut bits = vec![0u64; grid.len().div_ceil(64)];
    let mut queue: Vec<NodeId> = Vec::new();
    let mark = |bits: &mut Vec<u64>, node: NodeId, queue: &mut Vec<NodeId>| {
        let (word, bit) = (node as usize / 64, node % 64);
        if bits[word] >> bit & 1 == 0 {
            bits[word] |= 1 << bit;
            queue.push(node);
        }
    };
    for spawn in &world.map.spawns {
        if let Some(node) = grid.locate(spawn.position, 1.5) {
            mark(&mut bits, node, &mut queue);
        }
    }
    if queue.is_empty() {
        return vec![u64::MAX; grid.len().div_ceil(64)];
    }
    while let Some(a) = queue.pop() {
        for (b, _) in grid.links(a) {
            mark(&mut bits, b, &mut queue);
        }
    }
    bits
}

fn centroid(points: &[Vec3]) -> Option<Vec3> {
    if points.is_empty() {
        return None;
    }
    let n = points.len() as f32;
    let sum = points.iter().fold(Vec3::ZERO, |a, &b| a + b);
    Some(Vec3::new(sum.x / n, sum.y / n, sum.z / n))
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;
    use mapdata::{MapData, RayTree, Spawn, WalkGrid};

    use crate::v026g::bots::body::tests::{floor, wall_x};

    /// An 80 x 40 m field: side 0 spawns at x = 5, side 1 at x = 75, a
    /// wall across x = 40 from z = 0 to 25 (the way round is past z = 25).
    pub fn field() -> Arc<World> {
        field_with(|_| {})
    }

    /// [`field`] with its map data changed by `edit` (containers,
    /// batteries).
    pub fn field_with(edit: impl FnOnce(&mut MapData)) -> Arc<World> {
        field_with_solids(&[], edit)
    }

    /// [`field_with`] with `solids` (more triangles) in its geometry.
    pub fn field_with_solids(
        solids: &[mapdata::Triangle],
        edit: impl FnOnce(&mut MapData),
    ) -> Arc<World> {
        let mut triangles = floor(0.0, 0.0, 80.0, 40.0, 0.0).to_vec();
        triangles.extend(wall_x(40.0, 0.0, 25.0));
        triangles.extend_from_slice(solids);
        let grid = WalkGrid::build(
            &triangles,
            [0.0, 0.0, 80.0, 40.0],
            0.25,
            mapdata::Body::default(),
        );
        let spawn = |team: i32, x: f32, z: f32| Spawn {
            position: Vec3::new(x, 0.0, z),
            rotation: Vec3::ZERO,
            team,
            priority: 0,
            point_id: 0,
            layer: "default".into(),
        };
        let mut map = MapData {
            spawns: vec![
                spawn(0, 5.0, 10.0),
                spawn(0, 5.0, 20.0),
                spawn(1, 75.0, 10.0),
                spawn(1, 75.0, 20.0),
            ],
            ..MapData::default()
        };
        edit(&mut map);
        let sight = RayTree::build(&triangles);
        Arc::new(World::assemble(map, grid.into(), sight.into()))
    }

    #[test]
    fn routes_keep_out_of_the_other_sides_spawn_area_unless_bound_for_its_container() {
        let tactics = Tactics::new(field(), 1.0);
        // From side 0's spawn to beside side 1's area: out of its own, never
        // into theirs.
        let route = tactics
            .route(
                Vec3::new(5.0, 0.0, 10.0),
                Vec3::new(52.0, 0.0, 10.0),
                0,
                Some(0),
            )
            .unwrap();
        let mut s = 0.0;
        while s < route.length() {
            let at = route.point_at(s);
            assert!(!tactics.in_spawn_area(at, Some(1)), "{at:?}");
            s += 0.5;
        }
        // Bound for their container (inside their area): it may go in.
        let into = tactics
            .route_for(
                Vec3::new(5.0, 0.0, 10.0),
                Vec3::new(72.0, 0.0, 12.0),
                0,
                Some(0),
                true,
            )
            .unwrap();
        assert!(into.end().distance_xz(Vec3::new(72.0, 0.0, 12.0)) < 1.0);
        assert!(
            tactics
                .route(
                    Vec3::new(5.0, 0.0, 10.0),
                    Vec3::new(72.0, 0.0, 12.0),
                    0,
                    Some(0)
                )
                .is_none()
        );
        // A goal in their spawn area is no goal.
        assert!(!tactics.fine_goal(Vec3::new(75.0, 0.0, 15.0), 0, Some(0)));
        assert!(tactics.fine_goal(Vec3::new(40.0, 0.0, 35.0), 0, Some(0)));
        // The exits of side 1 are out of every spawn area, 30-60 m from it.
        let exits = tactics.exits(1);
        assert!(!exits.is_empty());
        for exit in exits {
            assert!(!tactics.in_spawn_area(*exit, None));
            assert!(exit.distance_xz(Vec3::new(75.0, 0.0, 15.0)) < 62.0);
        }
    }

    /// The base's build is fixed: a change to it (or to the searches its
    /// exits come from) fails here until [`TACTICS_ALGO`] is raised and
    /// the pin updated, so a stored base of the old build is never read.
    #[test]
    fn the_base_and_its_digest_are_fixed() {
        let base = TacticsBase::build(field(), 1.0);
        assert_eq!(base.digest(), PINNED_FIELD);
        assert_eq!(TACTICS_ALGO, 2);
    }

    const PINNED_FIELD: u64 = 736_835_410_510_453_169;

    #[test]
    fn a_written_base_reads_back_the_same_and_a_damaged_one_is_refused() {
        let dir = std::env::temp_dir().join(format!("srp-tactics-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(mapdata::built::tactics_file_name(9));
        let base = TacticsBase::build(field(), 1.0);
        base.write(&path, 9).unwrap();
        let read = TacticsBase::read(&path, 9, base.world.clone(), 1.0).unwrap();
        assert_eq!(read.digest(), base.digest());
        assert_eq!(read.exits(1), base.exits(1));
        assert!(TacticsBase::read(&path, 10, base.world.clone(), 1.0).is_err());
        // Read for another margin: the digest tells.
        assert!(TacticsBase::read(&path, 9, base.world.clone(), 2.0).is_err());
        let bytes = std::fs::read(&path).unwrap();
        for at in (0..bytes.len()).step_by(997).chain([bytes.len() - 1]) {
            let mut damaged = bytes.clone();
            damaged[at] ^= 0x41;
            std::fs::write(&path, &damaged).unwrap();
            assert!(
                TacticsBase::read(&path, 9, base.world.clone(), 1.0).is_err(),
                "byte {at}"
            );
        }
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn matches_share_one_base_and_keep_their_own_clocks() {
        let world = field();
        let base = Arc::new(TacticsBase::build(world.clone(), 1.0));
        assert_eq!(base.digest(), TacticsBase::build(world, 1.0).digest());
        assert_ne!(
            base.digest(),
            TacticsBase::build(base.world.clone(), 2.0).digest()
        );
        let (mut one, two) = (Tactics::of(base.clone()), Tactics::of(base.clone()));
        assert!(Arc::ptr_eq(one.base(), two.base()));
        assert_eq!(Arc::strong_count(&base), 3);
        // One match's clock is its own.
        one.hazards.pass_ms += 1;
        assert_ne!(one.hazards.pass_ms, two.hazards.pass_ms);
        // A match built alone answers as one on the shared base.
        let alone = Tactics::new(base.world.clone(), 1.0);
        assert_eq!(alone.base().digest(), base.digest());
        assert_eq!(alone.exits(1), two.exits(1));
    }

    #[test]
    fn spots_see_their_point_and_hide_from_another() {
        let tactics = Tactics::new(field(), 1.0);
        let target = Vec3::new(30.0, 0.0, 10.0);
        let from = Vec3::new(50.0, 0.0, 10.0);
        let query = SpotQuery {
            sight_to: Some(target),
            hidden_from: Some(from),
            samples: 200,
            ..SpotQuery::ring(target, 10.0, 25.0)
        };
        let spots = tactics.spots(&query, 0, 5, &mut Rng::new(5));
        assert!(!spots.is_empty());
        for spot in &spots {
            let d = spot.at.distance_xz(target);
            assert!((10.0..=25.0).contains(&d));
            // Behind the wall from the enemy's side: x < 40.
            assert!(spot.at.x < 40.0, "{spot:?}");
        }
    }

    /// A body inside a zone (always on) has no route to anywhere, but a
    /// way out: to the nearest floor it may walk on, a few metres off.
    #[test]
    fn a_body_in_a_fire_walks_out_to_the_nearest_floor_it_may() {
        use mapdata::{Rotation, Shape, ShapeKind, Zone, ZoneKind};
        let world = field_with(|map| {
            map.zones.push(Zone {
                kind: ZoneKind::Damage,
                name: "fire".into(),
                team: -1,
                position: Vec3::new(25.0, 0.0, 32.0),
                damage_type: "fire".into(),
                damage: None,
                shapes: vec![Shape {
                    kind: ShapeKind::Sphere,
                    center: Vec3::new(25.0, 1.0, 32.0),
                    rotation: Rotation::IDENTITY,
                    half: Vec3::new(3.0, 3.0, 3.0),
                }],
            });
        });
        let tactics = Tactics::new(world.clone(), 1.0);
        let grid = &world.grid;
        let inside = grid.locate(Vec3::new(25.0, 0.0, 32.0), 1.0).unwrap();
        assert!(!tactics.allowed(inside, 0, Some(0), false));
        let goal = Vec3::new(10.0, 0.0, 30.0);
        let to = grid.locate(goal, 1.0).unwrap();
        assert!(
            tactics
                .route_nodes(inside, to, 0, Some(0), false, |_| true)
                .is_none()
        );
        let out = tactics.escape(inside, 0, Some(0), false).unwrap();
        let end = out.point_at(out.length());
        let end_node = grid.locate(end, 0.3).unwrap();
        assert!(tactics.allowed(end_node, 0, Some(0), false));
        // The zone and its margin are 4 m round its centre: out within 5 m.
        assert!(out.length() < 5.0, "{}", out.length());
        // From there the goal has a way.
        assert!(
            tactics
                .route_nodes(end_node, to, 0, Some(0), false, |_| true)
                .is_some()
        );
    }

    /// A roof one only drops from (3 m up; the jump is 1.3 m) is no goal:
    /// not walkable, no spot on it even preferring height, `fine_goal`
    /// says no; the ground beside it is all three.
    #[test]
    fn a_roof_no_walk_gets_to_is_no_goal() {
        let mut triangles = floor(0.0, 0.0, 80.0, 40.0, 0.0).to_vec();
        triangles.extend(floor(50.0, 10.0, 60.0, 20.0, 3.0));
        let grid = WalkGrid::build(
            &triangles,
            [0.0, 0.0, 80.0, 40.0],
            0.25,
            mapdata::Body::default(),
        );
        let world = field_with(|_| {});
        let map = world.map.clone();
        let world = Arc::new(World::assemble(
            map,
            grid.into(),
            RayTree::build(&triangles).into(),
        ));
        let tactics = Tactics::new(world.clone(), 1.0);
        let grid = &world.grid;
        let roof = grid.locate(Vec3::new(55.0, 3.0, 15.0), 1.0).unwrap();
        let ground = grid.locate(Vec3::new(40.0, 0.0, 30.0), 1.0).unwrap();
        assert!(!tactics.walkable(roof));
        assert!(tactics.walkable(ground));
        assert!(grid.path(roof, ground).is_some(), "one drops from it");
        assert!(!tactics.fine_goal(Vec3::new(55.0, 3.0, 15.0), 0, Some(0)));
        assert!(tactics.fine_goal(Vec3::new(40.0, 0.0, 30.0), 0, Some(0)));
        let query = SpotQuery {
            prefer_high: 1.0,
            side: Some(0),
            ..SpotQuery::ring(Vec3::new(55.0, 1.5, 15.0), 0.0, 8.0)
        };
        let spots = tactics.spots(&query, 0, 10, &mut Rng::new(1));
        assert!(!spots.is_empty());
        assert!(spots.iter().all(|spot| spot.at.y < 1.0), "{spots:?}");
    }
}
