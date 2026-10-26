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

use mapdata::grid::{MARGINAL_JUMP_M, STEP_UP_M};
use mapdata::regions::Regions;
use mapdata::{NodeId, Shape, Vec3, WalkGrid, World};

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

/// [`Tactics::ledge_ahead`]: a ledge to jump up is looked for from this far
/// ahead of the stuck body …
const LEDGE_NEAR_M: f32 = 0.2;
/// … to this far (a stuck body stands at the ledge's face, a world is up to
/// 0.7 s old at the press), metres …
const LEDGE_FAR_M: f32 = 1.0;
/// … and this far to either side of its facing, metres.
const LEDGE_ASIDE_M: f32 = 0.4;
/// A way out ends at least this far from where it starts, metres: past the
/// motor's arrival distance, so the body really walks out.
const ESCAPE_MIN_M: f32 = 1.5;
/// The ways out a body chooses from lie within this of the nearest, metres
/// of the way to them. GUESS: round a zone a few metres across, both sides
/// (the Cologne anomaly bench's comparison took 6).
const ESCAPE_BAND_M: f32 = 6.0;
/// Ways out are taken this far apart round a zone, metres. GUESS: a body's
/// width or two.
const ESCAPE_SPACING_M: f32 = 1.0;
/// A metre of the way out inside the zone weighs as this many metres of
/// the way on to the goal. GUESS (the bench's comparison took 3).
const ESCAPE_IN_WEIGHT: f32 = 3.0;
/// A body walks out of a zone at least this fast, m/s: a walk aiming
/// forward (`kin`: 3.0).
const ESCAPE_WALK_MPS: f32 = 3.0;
/// A zone whose hit a second is at most this, of a body's health (`[0, 1]`,
/// as `BodyView::health` and the zones' hits), may be walked on through to a
/// way out further on while on. GUESS: under a tenth of a body's health in a
/// crossing of 2 s. It was 5.0, as if in health points: every zone of the
/// maps but one (Mamayev Kurgan's 120 a second) was mild, those of 0.8-1.2
/// a second too, a body's death in about a second.
const MILD_HIT_PS: f32 = 0.05;

/// Whether a zone that hits `hit_per_s` a second is mild ([`MILD_HIT_PS`]).
fn mild(hit_per_s: f32) -> bool {
    hit_per_s <= MILD_HIT_PS
}
/// A held route is looked over this far ahead for zones it would meet on,
/// metres (GUESS: 3 s of a sprint, past the time a zone's prediction is
/// sure).
const HELD_AHEAD_M: f32 = 15.0;
/// ... at points this far apart along it, metres (two floors).
const HELD_STEP_M: f32 = 0.5;
/// A region of a zone's floors is tried at distances this far apart,
/// metres (0.4 s of a sprint, a tenth of [`PASS_MS`]).
const REGION_STEP_M: f32 = 2.0;
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
    /// [`Self::walk_estimate`]'s distances by goal region, once made: they
    /// depend on the map alone.
    walks: std::sync::Mutex<std::collections::HashMap<u32, Arc<Vec<f32>>>>,
    /// [`Self::spots_kept`]'s lists.
    spot_lists: std::sync::Mutex<Vec<KeptSpots>>,
    /// [`Self::exit_of`]'s field, once made.
    exit_field: std::sync::Mutex<Option<ExitField>>,
    /// The teams' escapes from pockets that worked, and their trapped bots
    /// (`escape.rs`), for the match.
    pub escapes: super::escape::Board,
}

/// [`Tactics::exit_of`]'s field: each marked floor's way out.
type ExitField = Arc<std::collections::HashMap<NodeId, NodeId>>;

/// A list of [`Tactics::spots_kept`]: (key, kept until, the spots).
type KeptSpots = (u64, u32, Arc<Vec<Vec3>>);

/// The label's bits for the sides' spawn areas.
const SPAWN_BITS: u32 = 8;

/// The version of what [`TacticsBase::build`] gives, in its key: raised
/// with any change that changes the tactics of a map (the hazard marks,
/// the spawn areas, the regions, the exits and the searches they come
/// from), so a stored base of the old build is never read for the new.
/// `the_base_and_its_digest_are_fixed` fails until then.
pub const TACTICS_ALGO: u32 = 3;

/// The grid's own A* over the regions' costs, in metres: the mean of its
/// length over the regions' sum, measured on School's walks to a battery a
/// storey up (1.10, 1.14, 1.00, 1.04, 1.18).
pub const REGION_WALK_FACTOR: f32 = 1.1;

/// The cost of a walk from each region to `goal` along the directed links:
/// a Dijkstra on the reversed graph, infinite where no way leads.
fn region_walks_to(regions: &Regions, goal: u32) -> Vec<f32> {
    use std::cmp::Reverse;
    let mut dist = vec![f32::INFINITY; regions.len()];
    dist[goal as usize] = 0.0;
    let mut heap = std::collections::BinaryHeap::new();
    heap.push(Reverse((0f32.to_bits(), goal)));
    while let Some(Reverse((bits, r))) = heap.pop() {
        let d = f32::from_bits(bits);
        if d > dist[r as usize] {
            continue;
        }
        for &p in regions.into(r) {
            let cost = regions
                .out(p)
                .iter()
                .filter(|&&(to, _)| to == r)
                .map(|&(_, cost)| cost)
                .fold(f32::INFINITY, f32::min);
            let next = d + cost;
            if next < dist[p as usize] {
                dist[p as usize] = next;
                heap.push(Reverse((next.to_bits(), p)));
            }
        }
    }
    dist
}

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
            walks: Default::default(),
            spot_lists: Default::default(),
            exit_field: Default::default(),
            escapes: Default::default(),
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

    /// The walk from floor `from` to floor `to` in metres, over the
    /// regions' directed links (a drop is short one way and long the
    /// other), times [`REGION_WALK_FACTOR`]: coarse (a region is a block of
    /// 32 cells), for telling a battery a storey up from one on the ground,
    /// not for a route. `None`: no way over the regions.
    pub fn walk_estimate(&self, from: NodeId, to: NodeId) -> Option<f32> {
        let regions = &self.base.regions;
        let goal = regions.of(to);
        let walks = {
            let mut kept = self
                .walks
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            kept.entry(goal)
                .or_insert_with(|| Arc::new(region_walks_to(regions, goal)))
                .clone()
        };
        let d = walks[regions.of(from) as usize];
        d.is_finite().then_some(d * REGION_WALK_FACTOR)
    }

    /// Whether a walk over the regions from `from`'s to `to`'s gets there
    /// with the regions zones on all match mark shut ([`Hazards::fixed_at`],
    /// as [`Self::past_fixed_zones`]) but those `open` gives open, off another
    /// side's spawn area: a crossing of such zones is then the only way
    /// (`crossing::wanted`). Not when it gets there with them shut too (the
    /// plain search failed for something else), nor when it does not with
    /// them open. A few thousand regions at most, twice.
    pub fn only_across_fixed(
        &self,
        from: NodeId,
        to: NodeId,
        side: Option<i32>,
        objective: bool,
        open: impl Fn(NodeId) -> bool,
    ) -> bool {
        let regions = &self.base.regions;
        let (start, goal) = (regions.of(from), regions.of(to));
        let reach = |pass: &dyn Fn(NodeId) -> bool| {
            let mut seen = vec![false; regions.len()];
            seen[start as usize] = true;
            let mut queue = vec![start];
            while let Some(a) = queue.pop() {
                if a == goal {
                    return true;
                }
                for &(b, _) in regions.out(a) {
                    let middle = regions.middle(b);
                    if !seen[b as usize]
                        && (b == goal || (self.spawn_ok(middle, side, objective) && pass(middle)))
                    {
                        seen[b as usize] = true;
                        queue.push(b);
                    }
                }
            }
            false
        };
        !reach(&|middle| !self.hazards.fixed_at(middle))
            && reach(&|middle| !self.hazards.fixed_at(middle) || open(middle))
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
        self.hazards.allowed(node, time) && self.spawn_ok(node, side, objective)
    }

    /// [`Self::allowed`] for a body `from` now: a zone's floor by the
    /// soonest it gets there ([`Hazards::allowed_arriving`], the line's
    /// length a lower bound on the way's), so a route is not planned
    /// through a zone that will be on when the body comes to it, and is
    /// through one on now that will be off by then.
    pub fn allowed_from(
        &self,
        node: NodeId,
        time: u32,
        side: Option<i32>,
        objective: bool,
        from: Vec3,
    ) -> bool {
        let zoned = self.hazards.zone_of(node).is_some();
        (!zoned || {
            let d = self.world.grid.position(node).distance_xz(from);
            self.hazards.allowed_arriving(node, time, d)
        }) && self.spawn_ok(node, side, objective)
    }

    /// Not the other side's spawn area for a body of map side `side`,
    /// unless bound for their base (`objective`).
    pub fn spawn_ok(&self, node: NodeId, side: Option<i32>, objective: bool) -> bool {
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

    /// [`Self::allowed_from`] for a whole region: open when any of its
    /// floors may be, so the regions' search never answers "no way" where
    /// the floors' would find one (`Regions::route_search`). Its floors
    /// share their zones but lie at different distances: the zones' bounds
    /// give their range, tried every [`REGION_STEP_M`].
    fn region_allowed_from(
        &self,
        region: u32,
        time: u32,
        side: Option<i32>,
        objective: bool,
        from: Vec3,
    ) -> bool {
        let middle = self.base.regions.middle(region);
        if !self.spawn_ok(middle, side, objective) {
            return false;
        }
        let Some(zone) = self.hazards.zone_of(middle) else {
            return true;
        };
        let (near, far) = self.zone_distances(zone.zone, from);
        let steps = ((far - near) / REGION_STEP_M).ceil().clamp(0.0, 64.0) as u32;
        (0..=steps).any(|k| {
            let d = (near + k as f32 * REGION_STEP_M).min(far);
            self.hazards.allowed_arriving(middle, time, d)
        })
    }

    /// The nearest and farthest a floor a zone (index into
    /// `MapData::zones`) marks may lie from `from`, in a line: its shapes'
    /// bounds, widened by the hazard margin.
    fn zone_distances(&self, zone: usize, from: Vec3) -> (f32, f32) {
        let margin = self.hazards.margin;
        let shapes = &self.world.map.zones[zone].shapes;
        let (mut lo, mut hi) = ([f32::MAX; 2], [f32::MIN; 2]);
        for (a, b) in shapes.iter().map(Shape::bounds) {
            lo = [lo[0].min(a.x - margin), lo[1].min(a.z - margin)];
            hi = [hi[0].max(b.x + margin), hi[1].max(b.z + margin)];
        }
        if lo[0] > hi[0] {
            return (0.0, 0.0);
        }
        let dx = (lo[0] - from.x).max(from.x - hi[0]).max(0.0);
        let dz = (lo[1] - from.z).max(from.z - hi[1]).max(0.0);
        let fx = (from.x - lo[0]).abs().max((hi[0] - from.x).abs());
        let fz = (from.z - lo[1]).abs().max((hi[1] - from.z).abs());
        (dx.hypot(dz), fx.hypot(fz))
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
        let at = self.world.grid.position(start);
        path::plan_start(
            &self.world.grid,
            &self.base.regions,
            start,
            goal,
            |node| self.allowed_from(node, time, side, objective, at) && open(node),
            |region| self.region_allowed_from(region, time, side, objective, at),
            iters,
        )
    }

    /// The way so far of a [`Self::start_route`] search under way
    /// ([`path::Planning::partial`]), with the arguments it started with
    /// (`start`: its start floor).
    pub fn partial_route(
        &self,
        planning: &path::Planning,
        start: NodeId,
        time: u32,
        side: Option<i32>,
        objective: bool,
        open: impl Fn(NodeId) -> bool,
    ) -> Option<Route> {
        let at = self.world.grid.position(start);
        planning.partial(&self.world.grid, |node| {
            self.allowed_from(node, time, side, objective, at) && open(node)
        })
    }

    /// A [`Self::start_route`] search goes on: the route, or none, once it
    /// ends; `None` while it goes on. The floors let through must be those
    /// it started with: the same arguments (`start`: its start floor), and
    /// the hazards as they were ([`Hazards::version`]).
    #[allow(clippy::too_many_arguments)]
    pub fn resume_route(
        &self,
        planning: &mut path::Planning,
        start: NodeId,
        time: u32,
        side: Option<i32>,
        objective: bool,
        open: impl Fn(NodeId) -> bool,
        iters: &mut usize,
    ) -> Option<Option<Route>> {
        let at = self.world.grid.position(start);
        planning.resume(
            &self.world.grid,
            &self.base.regions,
            |node| self.allowed_from(node, time, side, objective, at) && open(node),
            |region| self.region_allowed_from(region, time, side, objective, at),
            iters,
        )
    }

    /// [`Self::start_route`] with the floors `crossable` gives open as well
    /// (floors of zones on all match a bot may walk across,
    /// `crossing::crossable`), off another side's spawn area all the same: a
    /// crossing's search, after the plain one found no way or a long one
    /// (`crossing.rs`). Regions are let through but for another side's spawn
    /// area: the floors decide.
    #[allow(clippy::too_many_arguments)]
    #[allow(clippy::result_large_err)]
    pub fn start_crossing(
        &self,
        start: NodeId,
        goal: NodeId,
        time: u32,
        side: Option<i32>,
        objective: bool,
        crossable: impl Fn(NodeId) -> bool,
        iters: &mut usize,
    ) -> Result<Option<Route>, path::Planning> {
        let at = self.world.grid.position(start);
        path::plan_start(
            &self.world.grid,
            &self.base.regions,
            start,
            goal,
            |node| self.crossing_allowed(node, time, side, objective, at, &crossable),
            |region| self.spawn_ok(self.base.regions.middle(region), side, objective),
            iters,
        )
    }

    /// A [`Self::start_crossing`] search goes on, as
    /// [`Self::resume_route`] (the same arguments, `crossable` too).
    #[allow(clippy::too_many_arguments)]
    pub fn resume_crossing(
        &self,
        planning: &mut path::Planning,
        start: NodeId,
        time: u32,
        side: Option<i32>,
        objective: bool,
        crossable: impl Fn(NodeId) -> bool,
        iters: &mut usize,
    ) -> Option<Option<Route>> {
        let at = self.world.grid.position(start);
        planning.resume(
            &self.world.grid,
            &self.base.regions,
            |node| self.crossing_allowed(node, time, side, objective, at, &crossable),
            |region| self.spawn_ok(self.base.regions.middle(region), side, objective),
            iters,
        )
    }

    /// A crossing's floors: [`Self::allowed_from`]'s, and those `crossable`
    /// gives off another side's spawn area.
    fn crossing_allowed(
        &self,
        node: NodeId,
        time: u32,
        side: Option<i32>,
        objective: bool,
        from: Vec3,
        crossable: &impl Fn(NodeId) -> bool,
    ) -> bool {
        self.allowed_from(node, time, side, objective, from)
            || (crossable(node) && self.spawn_ok(node, side, objective))
    }

    /// Whether the floors a body on `from` may walk to
    /// ([`Self::allowed_from`]) are at most `max` and walled by zones on all
    /// match: every floor refused at their edge is one only such zones mark
    /// ([`Hazards::only_fixed_at`]), none another side's spawn area or a zone
    /// that switches. A bot that fell or was pushed into such a patch has no
    /// route anywhere (Mamayev Kurgan's fire soft damage, 0.04 a second,
    /// rings 92 floors below a Cut spot).
    pub fn ringed(
        &self,
        from: NodeId,
        time: u32,
        side: Option<i32>,
        objective: bool,
        max: usize,
    ) -> bool {
        let grid = &self.world.grid;
        let at = grid.position(from);
        let mut seen = std::collections::HashSet::from([from]);
        let mut queue = vec![from];
        let mut walled = false;
        while let Some(n) = queue.pop() {
            for (b, _) in grid.links(n) {
                if seen.contains(&b) {
                    continue;
                }
                if self.allowed_from(b, time, side, objective, at) {
                    if seen.len() >= max {
                        return false;
                    }
                    seen.insert(b);
                    queue.push(b);
                } else if self.hazards.only_fixed_at(b) && self.spawn_ok(b, side, objective) {
                    walled = true;
                } else {
                    return false;
                }
            }
        }
        walled
    }

    /// The way out for a body on `from`, a floor it may not walk on now
    /// (inside a zone it was pushed or stepped into, or one about to come
    /// on): to a floor at least [`ESCAPE_MIN_M`] off that it may walk on
    /// and every floor next to it too (clear of the zone's edge), over any
    /// floors. No route to any goal starts there, so without it the body
    /// would stand in the fire.
    ///
    /// Which way out: the ways out within [`ESCAPE_BAND_M`] of the nearest,
    /// one a metre round the zone ([`ESCAPE_SPACING_M`]).
    /// - With no `goal`, the nearest.
    /// - With a goal, the one with the least way in the zone (×
    ///   [`ESCAPE_IN_WEIGHT`]) plus the line from it to the goal. A way out
    ///   other than the nearest only when the zone stays off till the body
    ///   is out at a walk ([`ESCAPE_WALK_MPS`]), or it is mild
    ///   ([`MILD_HIT_PS`]): forward through a zone that is off, back out of
    ///   one that is on. The Cologne anomaly bench (runs 2b and 8): the
    ///   nearest way out alone went back the way it came, then in again.
    /// - None within [`ESCAPE_FLOORS`] (deep in a big zone): the way out
    ///   the zone's exit field gives ([`Self::exit_of`]).
    pub fn escape(
        &self,
        from: NodeId,
        goal: Option<Vec3>,
        time: u32,
        side: Option<i32>,
        objective: bool,
    ) -> Option<Route> {
        let grid = &self.world.grid;
        let mut seen = std::collections::HashSet::from([from]);
        let mut queue = std::collections::VecDeque::from([(from, 0.0f32)]);
        let start = grid.position(from);
        let ok = |n: NodeId| self.allowed(n, time, side, objective);
        let mut first: Option<f32> = None;
        // The ways out, each with the way to it.
        let mut outs: Vec<(NodeId, f32)> = Vec::new();
        while let Some((a, d)) = queue.pop_front() {
            if first.is_some_and(|first| d > first + ESCAPE_BAND_M) {
                break;
            }
            let at = grid.position(a);
            if at.distance_xz(start) >= ESCAPE_MIN_M && ok(a) && grid.links(a).all(|(b, _)| ok(b)) {
                first.get_or_insert(d);
                if !outs
                    .iter()
                    .any(|&(out, _)| grid.position(out).distance_xz(at) < ESCAPE_SPACING_M)
                {
                    outs.push((a, d));
                }
                continue;
            }
            if seen.len() > ESCAPE_FLOORS {
                break;
            }
            for (b, _) in grid.links(a) {
                if grid.plans(a, b) && seen.insert(b) {
                    queue.push_back((b, d + grid.position(b).distance_xz(at)));
                }
            }
        }
        let nearest = outs.iter().copied().min_by(|a, b| a.1.total_cmp(&b.1));
        // Out of a spawn area (no zone there): the nearest way, as ever.
        let goal = goal.filter(|_| self.hazards.zone_of(from).is_some());
        let out = match (goal, nearest) {
            (Some(goal), Some(nearest)) => {
                let mild = mild(self.hazards.hit_per_s(from));
                let in_time = |d: f32| {
                    let walk_ms = (d / ESCAPE_WALK_MPS * 1000.0) as u32;
                    self.hazards.allowed_for(from, time, walk_ms)
                };
                let score = |&(out, d): &(NodeId, f32)| {
                    d * ESCAPE_IN_WEIGHT + grid.position(out).distance_xz(goal)
                };
                outs.iter()
                    .filter(|&&(out, d)| out == nearest.0 || mild || in_time(d))
                    .min_by(|a, b| score(a).total_cmp(&score(b)))
                    .map(|&(out, _)| out)
            }
            (None, nearest) => nearest.map(|(out, _)| out),
            (Some(_), None) => None,
        };
        #[cfg(test)]
        escapes::count(out.is_some(), out.is_some() && out != nearest.map(|n| n.0));
        let out = out.or_else(|| self.exit_of(from))?;
        path::plan(grid, from, out)
    }

    /// The floor no zone marks that the exit field leads to from `from`, a
    /// floor a zone marks: the field is a breadth-first search inward from
    /// the unmarked floors at the zones' edges over the marked ones, made
    /// once a match when first wanted (a few ms on the largest maps' tens
    /// of thousands of marked floors). For a body deeper in a zone than
    /// [`Self::escape`]'s search reaches (the Cologne bench's run 5: 25 m
    /// into a big zone, 73 escapes found nothing and it stood 37 s).
    pub fn exit_of(&self, from: NodeId) -> Option<NodeId> {
        let field = {
            let mut cache = self
                .exit_field
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            cache
                .get_or_insert_with(|| Arc::new(self.exit_field()))
                .clone()
        };
        field.get(&from).copied()
    }

    /// [`Self::exit_of`]'s field: each marked floor's way out.
    fn exit_field(&self) -> std::collections::HashMap<NodeId, NodeId> {
        let grid = &self.world.grid;
        let marked = |n: NodeId| self.hazards.set_of(n) != 0;
        let mut field = std::collections::HashMap::new();
        // The marked floors each marked floor is a link from.
        let mut into: std::collections::HashMap<NodeId, Vec<NodeId>> =
            std::collections::HashMap::new();
        let mut queue = std::collections::VecDeque::new();
        for a in (0..grid.len() as NodeId).filter(|&a| marked(a)) {
            for (b, _) in grid.links(a) {
                if !grid.plans(a, b) {
                    continue;
                }
                if !marked(b) {
                    if let std::collections::hash_map::Entry::Vacant(entry) = field.entry(a) {
                        entry.insert(b);
                        queue.push_back(a);
                    }
                } else {
                    into.entry(b).or_default().push(a);
                }
            }
        }
        while let Some(b) = queue.pop_front() {
            let out = field[&b];
            for &a in into.get(&b).map_or(&[][..], Vec::as_slice) {
                if let std::collections::hash_map::Entry::Vacant(entry) = field.entry(a) {
                    entry.insert(out);
                    queue.push_back(a);
                }
            }
        }
        field
    }

    /// Whether the route a body at `feet` holds is shut ahead: a floor a
    /// zone marks within [`HELD_AHEAD_M`] along it is not open when the
    /// body can get there ([`Self::allowed_from`]'s test). A route is
    /// planned for the zones as they were then; a zone that came on since,
    /// or one the body is late for, would be walked into (the Cologne
    /// anomaly bench, runs 1 and 3: 1.2 s on a zone's floors while on).
    ///
    /// On a crossing (`crossing`, `crossing.rs`), the floors only zones on all
    /// match mark do not shut it: the crossing was weighed when planned.
    pub fn route_shut_ahead(
        &self,
        route: &mut Route,
        feet: Vec3,
        time: u32,
        crossing: bool,
    ) -> bool {
        if self.hazards.zones.is_empty() {
            return false;
        }
        let grid = &self.world.grid;
        // Walking out of a zone (its way out goes through it): not looked
        // over till it is out.
        if grid
            .locate(feet, 1.5)
            .is_some_and(|node| !self.hazards.allowed(node, time))
        {
            return false;
        }
        let (from, _) = route.progress(feet);
        let to = (from + HELD_AHEAD_M).min(route.length());
        let steps = ((to - from) / HELD_STEP_M).ceil().max(0.0) as u32;
        (0..=steps).any(|k| {
            let point = route.floor_point((from + k as f32 * HELD_STEP_M).min(to));
            grid.locate(point, 0.6).is_some_and(|node| {
                self.hazards.zone_of(node).is_some()
                    && !(crossing && self.hazards.only_fixed_at(node))
                    && !self
                        .hazards
                        .allowed_arriving(node, time, point.distance_xz(feet))
            })
        })
    }

    /// Whether a step from `a` to `b` keeps off the zones' floors that are
    /// not open now ([`Hazards::allowed`]): walls and edges are not looked
    /// at (the stuck watch's keys are for those).
    pub fn zone_free_step(&self, a: Vec3, b: Vec3, time: u32) -> bool {
        let grid = &self.world.grid;
        (1..=3).all(|k| {
            let point = a + (b - a) * (k as f32 / 3.0);
            grid.locate(point, 1.0)
                .is_none_or(|node| self.hazards.allowed(node, time))
        })
    }

    /// The rise of the lowest ledge a stuck body at `feet` facing `yaw` can
    /// jump up, if there is one ahead: a floor to stand on more than the
    /// client's step-up ([`STEP_UP_M`], which it walks up by itself) and at
    /// most [`MARGINAL_JUMP_M`] above the feet (under the standing jump's top,
    /// [`mapdata::grid::STANDING_JUMP_M`]), [`LEDGE_NEAR_M`]..[`LEDGE_FAR_M`]
    /// ahead along `yaw` and within [`LEDGE_ASIDE_M`] to a side. The
    /// unstick's forward jump (`Bots::fixed`): a body held by a ledge it
    /// walks at gets up it, any other stop backs off with no jump.
    pub fn ledge_ahead(&self, feet: Vec3, yaw: f32) -> Option<f32> {
        let grid = &self.world.grid;
        let [fx, fz] = super::kin::forward(yaw);
        let reach = LEDGE_FAR_M + LEDGE_ASIDE_M;
        grid.floors_in([
            feet.x - reach,
            feet.z - reach,
            feet.x + reach,
            feet.z + reach,
        ])
        .filter_map(|node| {
            let at = grid.position(node);
            let (ex, ez) = (at.x - feet.x, at.z - feet.z);
            let ahead = ex * fx + ez * fz;
            let across = (ex * fz - ez * fx).abs();
            let rise = at.y - feet.y;
            ((LEDGE_NEAR_M..=LEDGE_FAR_M).contains(&ahead)
                && across <= LEDGE_ASIDE_M
                && rise > STEP_UP_M
                && rise <= MARGINAL_JUMP_M
                && grid.can_stand(node))
            .then_some(rise)
        })
        .min_by(f32::total_cmp)
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
        let Some((from, to)) = line_ends(grid, a, b) else {
            return false;
        };
        path::straight_where(grid, from, to, |node| {
            self.allowed(node, time, side, objective)
        })
    }

    /// [`Self::clear_line_for`] over zones' floors too, those that switch
    /// as well as those on all match (the spawn rule still holds): the
    /// caller weighs every zone's damage (a battery's crossing, `collect`).
    /// No pocket or fall is checked either.
    pub fn clear_line_through_zones(
        &self,
        a: Vec3,
        b: Vec3,
        side: Option<i32>,
        objective: bool,
    ) -> bool {
        let grid = &self.world.grid;
        let Some((from, to)) = line_ends(grid, a, b) else {
            return false;
        };
        path::straight_where(grid, from, to, |node| self.spawn_ok(node, side, objective))
    }

    /// [`Self::clear_line_for`] for the fight's side step: nor into a
    /// pocket with no standing-jump way out (`escape::standing_exit`) the
    /// bot at `a` is not in already. A step off a rim into a pit it never
    /// gets out of costs the bot the match; in its own pocket it steps as
    /// it likes. Once bots sprint, a pocket with a sprint exit counts too,
    /// for one not carrying. Nor onto a floor near an unwalled fall
    /// (`WalkGrid::near_fall`): a stride's overshoot there goes over the
    /// edge (a third of the falls off route that keeping such edges
    /// added were side steps: `player_bots_mapdata.md`, "Grid: walls at
    /// the body's height").
    pub fn clear_step_for(
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
        let home = grid.pocket_of(from);
        let trap = |node: NodeId| {
            grid.near_fall(node)
                || grid.pocket_of(node).is_some_and(|pocket| {
                    Some(pocket) != home
                        && !super::escape::standing_exit(grid, &self.escapes, pocket)
                })
        };
        !trap(to)
            && path::straight_where(grid, from, to, |node| {
                self.allowed(node, time, side, objective) && !trap(node)
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

    /// The spots `find` gives for `key`, made once and kept until `now +
    /// keep_ms` for every bot that asks: a play that offers itself only
    /// where it has a spot (HoldExit, Crossfire) looks at each poll, and
    /// [`Self::spots`]'s sight rays are too dear for that.
    pub fn spots_kept(
        &self,
        key: u64,
        now: u32,
        keep_ms: u32,
        find: impl FnOnce() -> Vec<Vec3>,
    ) -> Arc<Vec<Vec3>> {
        let mut lists = self.spot_lists.lock().unwrap_or_else(|e| e.into_inner());
        lists.retain(|&(_, until, _)| now < until);
        if let Some((_, _, spots)) = lists.iter().find(|(k, ..)| *k == key) {
            return spots.clone();
        }
        let spots = Arc::new(find());
        lists.push((key, now.saturating_add(keep_ms), spots.clone()));
        spots
    }

    /// A key for [`Self::spots_kept`]: the play's `tag` (its kind, under
    /// 64, plus 64 times a variant of its own up to 3), the side and the
    /// point the spots are round, to `KEY_GRID_M`.
    pub fn spot_key(tag: u32, side: Option<i32>, around: Vec3) -> u64 {
        const KEY_GRID_M: f32 = 4.0;
        let cell = |v: f32| u64::from((v / KEY_GRID_M).round() as i32 as u16);
        u64::from(tag) << 56
            | u64::from(side.map_or(0xff, |s| s as u8)) << 48
            | cell(around.x) << 32
            | cell(around.y) << 16
            | cell(around.z)
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
            if grid.plans(a, b) {
                mark(&mut bits, b, &mut queue);
            }
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

/// The floors a line from `a` to `b` runs between ([`Tactics::clear_line_for`]):
/// `a`'s, and the floor under `b`, in its cell
/// (the nearest in height, in the window [`WalkGrid::locate`] takes). The
/// nearest floor round `b` is not its end when none is under it: a jump's
/// run-up 0.8 m behind its take-off at the end of a corridor one floor wide
/// and on the diagonal was over no floor, `locate` found the bot's own floor
/// 0.55 m from it, so the line to it was clear and the bot walked at it for
/// every try (level_06 day 4's pocket at (92.1, 139.4)).
fn line_ends(grid: &WalkGrid, a: Vec3, b: Vec3) -> Option<(NodeId, NodeId)> {
    let from = grid.locate(a, 1.0)?;
    let dy = |n: NodeId| grid.position(n).y - b.y;
    let to = grid
        .floors_in([b.x, b.z, b.x, b.z])
        .filter(|&n| (-2.5..=1.0).contains(&dy(n)))
        .min_by(|&m, &n| dy(m).abs().total_cmp(&dy(n).abs()))?;
    Some((from, to))
}

/// The bench's counts of [`Tactics::escape`] (all matches of the run): the
/// ways out it found, of them not the nearest (on toward the goal), and
/// none found in its search.
#[cfg(test)]
pub mod escapes {
    use std::sync::atomic::{AtomicU64, Ordering};

    static COUNTS: [AtomicU64; 3] = [const { AtomicU64::new(0) }; 3];

    pub fn count(found: bool, on: bool) {
        let k = if !found { 2 } else { usize::from(on) };
        COUNTS[k].fetch_add(1, Ordering::Relaxed);
    }

    /// (back the nearest way, on toward the goal, none in the search).
    pub fn counts() -> [u64; 3] {
        std::array::from_fn(|k| COUNTS[k].load(Ordering::Relaxed))
    }
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

    /// [`MILD_HIT_PS`] is of a body's health, `[0, 1]`: a zone that kills
    /// a whole body within a crossing of 2 s (Vostok Radar Station's and
    /// Cologne's 1.2 a second, Tarakanovsky Fort's 0.8) is not mild; one that
    /// takes under a tenth of it (Mamayev Kurgan's fire soft damage, 0.04)
    /// is.
    #[test]
    fn a_mild_zone_takes_under_a_tenth_of_health_in_a_crossing_of_2_s() {
        let per_s = |max_hit: f32| {
            mapdata::Damage {
                max_hit,
                parts: 2,
                continuous: true,
                ..mapdata::Damage::default()
            }
            .raw()
        };
        for deadly in [0.6, 0.4] {
            assert!(2.0 * per_s(deadly) >= 1.0);
            assert!(!mild(per_s(deadly)), "{}", per_s(deadly));
        }
        assert!(2.0 * per_s(0.02) <= 0.1);
        assert!(mild(per_s(0.02)));
    }

    /// F4: a stuck body finds the ledge it can jump up (more than the
    /// client's step of 0.6 m, at most 0.95 m) a metre ahead of it along its
    /// facing, and nothing else: not a step it walks up, not a ledge over
    /// the standing jump, not one behind it or far to a side.
    #[test]
    fn ledge_ahead_finds_only_a_ledge_a_standing_jump_gets_up() {
        let at_east = -std::f32::consts::FRAC_PI_2; // `kin::forward` = +x
        let ledge = |rise: f32| {
            let platform = floor(30.0, 0.0, 40.0, 40.0, rise);
            Tactics::new(field_with_solids(&platform, |_| {}), 1.0)
        };
        let feet = Vec3::new(29.4, 0.0, 10.0);
        let near = |rise: f32| ledge(rise).ledge_ahead(feet, at_east);
        let got = near(0.8).expect("a ledge of 0.8 m");
        assert!((got - 0.8).abs() < 0.05, "{got}");
        assert!(near(0.5).is_none(), "a step the client walks up");
        assert!(near(1.2).is_none(), "over the standing jump");
        assert!(near(3.0).is_none(), "a wall");
        let tactics = ledge(0.8);
        let west = std::f32::consts::FRAC_PI_2;
        assert!(tactics.ledge_ahead(feet, west).is_none(), "behind it");
        assert!(
            tactics
                .ledge_ahead(Vec3::new(27.0, 0.0, 10.0), at_east)
                .is_none(),
            "2.6 m off"
        );
        assert!(
            tactics.ledge_ahead(feet, std::f32::consts::PI).is_none(),
            "to the side of its facing"
        );
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
        assert_eq!(TACTICS_ALGO, 3);
    }

    const PINNED_FIELD: u64 = 8_340_154_445_228_000_430;

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

    /// A slab 6 m over the field (deeper than the harmless drop), its edges
    /// kept: the side step keeps off them, inside it steps.
    #[test]
    fn no_side_step_onto_an_edge_over_a_fall() {
        let world = field_with_solids(&floor(50.0, 30.0, 56.0, 36.0, 6.0), |_| {});
        let tactics = Tactics::new(world.clone(), 1.0);
        let grid = &world.grid;
        let middle = Vec3::new(53.0, 6.0, 33.0);
        let edge = Vec3::new(55.9, 6.0, 33.0);
        assert!(grid.near_fall(grid.locate(edge, 0.3).unwrap()));
        assert!(!grid.near_fall(grid.locate(middle, 0.3).unwrap()));
        assert!(!tactics.clear_step_for(middle, edge, 0, Some(0), false));
        assert!(tactics.clear_step_for(middle, Vec3::new(54.0, 6.0, 33.0), 0, Some(0), false));
        // The walk there is still a line on allowed floors.
        assert!(tactics.clear_line_for(middle, edge, 0, Some(0), false));
    }

    /// A line's end over no floor is not clear, though a floor is within
    /// the 0.6 m `locate` looks: a jump's run-up off the floors was walked
    /// at for every try (level_06 day 4's pocket at (92.1, 139.4)).
    #[test]
    fn a_line_to_a_point_over_no_floor_is_not_clear() {
        let world = field_with_solids(&floor(50.0, 30.0, 56.0, 36.0, 6.0), |_| {});
        let tactics = Tactics::new(world.clone(), 1.0);
        let grid = &world.grid;
        let middle = Vec3::new(53.0, 6.0, 33.0);
        let past = Vec3::new(56.3, 6.0, 33.0);
        let by = grid
            .locate(past, 0.6)
            .expect("the slab's edge is within reach");
        assert!(grid.position(by).x < 56.0);
        assert!(!tactics.clear_line_for(middle, past, 0, Some(0), false));
        assert!(tactics.clear_line_for(middle, Vec3::new(55.9, 6.0, 33.0), 0, Some(0), false));
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
        let out = tactics.escape(inside, None, 0, Some(0), false).unwrap();
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
