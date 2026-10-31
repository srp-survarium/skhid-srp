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

mod progress;

use super::hazards::{HazardMarks, Hazards, PASS_MS};
use super::path::{self, Route};
use super::route_filter::RouteFilter;
use super::rng::Rng;
use super::tuning::CoverRule;
use super::tuning::Exposure;
use super::view::still_eye;

/// A spawn area: this far round a side's spawn points, metres (GUESS from
/// the maps: spawn points of a side lie in a few clusters 10-20 m across).
pub const SPAWN_AREA_M: f32 = 20.0;
/// A point is looked for on the grid within this, metres.
const LOCATE_M: f32 = 3.0;
/// A body on refused floors may test a line over them this much beyond the way to the nearest allowed floor
/// ([`Tactics::zone_exit_reach`]), metres: the turns of a way out, a stride's overshoot.
const ZONE_EXIT_SLACK_M: f32 = 1.0;
/// … and looks for that floor within this height of its feet (one storey) and this far, metres.
const ZONE_EXIT_STOREY_M: f32 = 2.5;
const ZONE_EXIT_MAX_M: f32 = 64.0;
/// How much height a spot earns [`SpotQuery::prefer_high`] for, metres: the
/// reach of the old search (the grid's `locate` finds floors up to 2 m over
/// the ring's point), so a spot on a storey above ranks with the best of the
/// floor below, and what puts it first is its sight of the point, not its
/// height.
const HIGH_CREDIT_M: f32 = 2.0;
/// Floors of one storey are this far apart at least, metres: a spot search
/// takes one floor a band of this height of a column.
const STOREY_BAND_M: f32 = 2.0;
/// How far round a ring's probe a storey above is looked for, metres.
const STOREY_RADIUS_M: f32 = 1.0;
/// A spot higher than this over its anchor is on a storey above it, metres:
/// the old search's reach.
const STOREY_FROM_M: f32 = 2.0;
/// How far past the ring's outer edge the walk up to a storey may go, and
/// how many floors it may visit, metres and floors.
const WALK_UP_MARGIN_M: f32 = 20.0;
const WALK_UP_MAX_FLOORS: usize = 40_000;
/// A use reaches this far from the eye, metres: the client's ray is
/// `usable_objects_detection_distance`, 2.0 (PROVEN), and must hit the
/// object's own shape; 0.2 is kept for the aim and the body's sway.
pub const USE_REACH_M: f32 = 1.8;
/// A way out of a zone is looked for over at most this many floors.
const ESCAPE_FLOORS: usize = 20_000;
/// A step out of a grenade's blast ([`Tactics::step_out`]) looks over at most
/// this many floors: its reach is a few seconds' walk, about 14 m, a disc of
/// some 10,000 floors at the grid's 0.25 m.
const STEP_OUT_FLOORS: usize = 12_000;
/// With no floor outside the blast in reach, the one as far as it gets is
/// taken only if it is this much farther than where the body stands, metres.
const STEP_OUT_GAIN_M: f32 = 0.5;
/// How many floors outside a blast [`Tactics::step_out_routes`] plans a route to, at most.
const STEP_OUT_CANDIDATES: usize = 6;

/// What [`Tactics::step_out_scan`] found.
struct StepOutScan {
    /// The floors outside the radius, in the order found.
    outside: Vec<NodeId>,
    /// The floor in reach farthest from the blast, and how far.
    farthest: Option<(NodeId, f32)>,
    /// How far the start is from the blast.
    start_far: f32,
}

/// Which floor [`Tactics::step_out_routes`] gave.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StepOutPick {
    /// The nearest floor outside the radius by link distance, its route within reach.
    Nearest,
    /// A later one: the nearer floors' routes were longer than the reach.
    Later,
    /// None outside within reach by its route: the farthest floor in reach.
    Farthest,
}

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
/// A zone's hit a second is read at most this, of a body's health, where
/// ways out are compared by it ([`Tactics::escape`]): a zone the map file
/// gives no hit for is infinite (`Hazard::hit_per_s`).
const ESCAPE_HIT_CAP: f32 = 1.0;
/// The floors [`Tactics::live_exit_of`] looks over for the way out of a live field: most of a big map's, for a field
/// wider than [`ESCAPE_FLOORS`] reaches (it runs only when the escape search found nothing).
const LIVE_EXIT_FLOORS: usize = 100_000;
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
    /// How far forward each region is, by the walk ([`progress`]): worked out
    /// at the first ask (`roam.join_ahead`), once for the map, not stored
    /// with the base; a map no switch asks it of never makes it.
    progress: std::sync::OnceLock<progress::Progress>,
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
    /// The places humans spend time high on this map, put on its floors, and
    /// their share of alive time (V-S2b); empty without the table's keys.
    pub perches: Vec<super::routes::Perch>,
    pub perch_share: Option<super::routes::PerchShare>,
    /// [`Self::past_fixed_zones`]'s regions, and the zones it was made for
    /// ([`Hazards::fixed_zones`]): made again only when they change (at
    /// the anomalies' attach).
    past_fixed: std::sync::Mutex<Option<PastFixed>>,
    /// [`Self::walk_estimate`]'s distances by goal region: they depend on
    /// the map alone; the most recent [`WALKS_MOST`] are kept.
    walks: std::sync::Mutex<WalkCache>,
    /// [`Self::spots_kept`]'s lists.
    spot_lists: std::sync::Mutex<Vec<KeptSpots>>,
    /// [`Self::walk_up_set`]'s sets, the latest [`WALK_UP_KEPT`]: they depend
    /// on the map alone and the point asked.
    walk_ups: std::sync::Mutex<Vec<(WalkUpKey, Arc<WalkUp>)>>,
    /// [`Self::exit_of`]'s field, once made.
    exit_field: std::sync::Mutex<Option<ExitField>>,
    /// The teams' escapes from pockets that worked, and their trapped bots
    /// (`escape.rs`), for the match.
    pub escapes: super::escape::Board,
    /// Whether a way out stays under its start's zone strength
    /// (`Tuning::hazard.escape`, [`Self::escape`]).
    pub escape: super::tuning::EscapeRule,
    /// The bushes a sight line crosses (`foliage.rs`); none without the file.
    pub foliage: Arc<super::foliage::Foliage>,
    /// The grass a sight line crosses (`grass.rs`); none without the file.
    pub grass: Arc<super::grass::Grass>,
    /// Whether the fight's side step is refused onto the band of a lip
    /// (`Tuning::aim.combat.clear_step_sees_lip`, [`Self::clear_step_for`]).
    pub clear_step_sees_lip: bool,
    /// What [`Self::shot_reaches`] answered, and the lines the pierce opened (the logs' counts).
    pub cover_tally: CoverTally,
    /// What the commander's gang-up cap counted (PR C item 20), by play.
    pub gang_tally: super::team::GangTally,
}

/// What [`Tactics::line_of_fire`] finds: whether the line is open to fire and the share of the bullet's damage that reaches its end.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Fire {
    pub open: bool,
    pub share: f32,
}

/// The verdicts of [`Tactics::shot_reaches`] and the lines it opened where the solid mesh had shut them.
#[derive(Debug, Default)]
pub struct CoverTally {
    /// Asked with the cover tuning off (answered as sight).
    pub plain: std::sync::atomic::AtomicU64,
    /// A clear line.
    pub open_clear: std::sync::atomic::AtomicU64,
    /// Open through cover the solid mesh counts as a wall (`opened`): over the tuning's share reached.
    pub open_through: std::sync::atomic::AtomicU64,
    /// Shut: some of the damage reached, no more than the tuning's share.
    pub shut_partial: std::sync::atomic::AtomicU64,
    /// Shut: the bullet stopped.
    pub shut_stopped: std::sync::atomic::AtomicU64,
    /// The first [`COVER_EVENTS`] of `open_through`, drained by the bench's event file.
    events: std::sync::Mutex<Vec<CoverEvent>>,
    /// Hiding-spot queries weighed by the share that reaches (`CoverRule::spot_cost`) that found a spot.
    pub spot_weighed: std::sync::atomic::AtomicU64,
    /// Of those, the ones where the weighed best is not the spot the filter alone would have given first (the
    /// nearest by the base score that passes).
    pub spot_changed: std::sync::atomic::AtomicU64,
    /// The share (thousandths) that reaches the first spot the filter alone would have given, summed over `spot_weighed`.
    pub spot_share_filter_milli: std::sync::atomic::AtomicU64,
    /// The share (thousandths) that reaches the spot given, summed over `spot_weighed`.
    pub spot_share_weighed_milli: std::sync::atomic::AtomicU64,
    /// [A195] Spot lists ranked with an [`Exposure`] term; those whose best is not the best by height and nearness; the
    /// bearings (other than the anchor's) of the best by height and nearness, and of the best given, summed over the lists.
    pub exposure_lists: std::sync::atomic::AtomicU64,
    pub exposure_changed: std::sync::atomic::AtomicU64,
    pub exposure_other_filter: std::sync::atomic::AtomicU64,
    pub exposure_other_weighed: std::sync::atomic::AtomicU64,
    spot_events: std::sync::Mutex<Vec<SpotEvent>>,
    /// All of them, drained or not.
    pub events_seen: std::sync::atomic::AtomicU64,
    /// Those kept for the logs, drained or not ([`COVER_EVENTS`] at most).
    events_kept: std::sync::atomic::AtomicU64,
}

/// A line the pierce opened: one note per event (`from` shoots at `to`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CoverEvent {
    pub from: Vec3,
    pub to: Vec3,
    /// The share of the damage that reaches, and the front faces it crossed.
    pub share: f32,
    pub crossed: u32,
}

/// A hiding spot the weighing changed: the spot the filter alone gives and the one given, and the share of the bullet
/// that reaches each (`around` is where the bot stands, `from` the enemy).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SpotEvent {
    pub around: Vec3,
    pub from: Vec3,
    pub filter: (Vec3, f32),
    pub weighed: (Vec3, f32),
}

/// Events kept for the logs per match.
const COVER_EVENTS: usize = 100;

/// Changed hiding spots kept for the logs per match.
const SPOT_EVENTS: usize = 300;

impl CoverTally {
    /// The events not yet drained.
    pub fn drain_events(&self) -> Vec<CoverEvent> {
        std::mem::take(&mut *self.events.lock().unwrap_or_else(|e| e.into_inner()))
    }

    /// The changed hiding spots not yet drained.
    pub fn drain_spot_events(&self) -> Vec<SpotEvent> {
        std::mem::take(&mut *self.spot_events.lock().unwrap_or_else(|e| e.into_inner()))
    }
}

/// [`Tactics::exit_of`]'s field: each marked floor's way out.
type ExitField = Arc<std::collections::HashMap<NodeId, NodeId>>;

/// A list of [`Tactics::spots_kept`]: (key, kept until, the spots).
type KeptSpots = (u64, u32, Arc<Vec<Vec3>>);

/// How many of [`Tactics::walk_up_set`]'s sets are kept (each is a bit a floor of the
/// grid, a few hundred KB on the biggest map).
const WALK_UP_KEPT: usize = 32;

/// The point and radius a [`WalkUp`] was made for, as bits.
type WalkUpKey = [u32; 4];

/// The floors a walk up from a point gets to ([`Tactics::walk_up_from`]): a bit
/// a floor of the grid, in place of a hash set (the profile of the spot
/// lists: 92 % of a build was this search, 6 ms of 5.2-5.7).
pub(super) struct WalkUp(Vec<u64>);

impl WalkUp {
    fn new(floors: usize) -> Self {
        Self(vec![0; floors.div_ceil(64)])
    }

    /// Adds `node`; whether it was not in.
    fn insert(&mut self, node: NodeId) -> bool {
        let (word, bit) = (node as usize >> 6, 1u64 << (node & 63));
        let was = self.0[word] & bit != 0;
        self.0[word] |= bit;
        !was
    }

    pub(super) fn contains(&self, node: NodeId) -> bool {
        self.0
            .get(node as usize >> 6)
            .is_some_and(|word| word >> (node & 63) & 1 == 1)
    }
}

/// The label's bits for the sides' spawn areas.
const SPAWN_BITS: u32 = 8;

/// The version of what [`TacticsBase::build`] gives, in its key: raised
/// with any change that changes the tactics of a map (the hazard marks,
/// the spawn areas, the regions, the exits and the searches
/// they come from), so a stored base of the old build is never read for the new.
/// `the_base_and_its_digest_are_fixed` fails until then.
pub const TACTICS_ALGO: u32 = 4;

/// How many goal regions' distances [`Tactics::walk_estimate`] keeps: each is
/// one `f32` a region (823-4,477 on the offered maps, 3-18 KB), so at most
/// 2.3 MB a match, against 80 MB for all of them.
pub const WALKS_MOST: usize = 128;

/// The kept distances to goal regions, oldest first out, and how often
/// they served ([`Tactics::walk_stats`]).
#[derive(Default)]
struct WalkCache {
    kept: std::collections::HashMap<u32, Arc<Vec<f32>>>,
    order: std::collections::VecDeque<u32>,
    hits: u64,
    misses: u64,
}

impl WalkCache {
    /// Keeps `walks` for `goal`, the oldest of more than `most` going out.
    fn keep(&mut self, goal: u32, walks: Arc<Vec<f32>>, most: usize) {
        if self.kept.insert(goal, walks).is_none() {
            self.order.push_back(goal);
            while self.order.len() > most {
                if let Some(old) = self.order.pop_front() {
                    self.kept.remove(&old);
                }
            }
        }
    }
}

/// What the walk cache has done: hits and misses, goals kept now, and the
/// bytes they hold.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct WalkStats {
    pub hits: u64,
    pub misses: u64,
    pub kept: usize,
    pub bytes: usize,
}

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

/// At most this many of the enemy's route points are cast to for one spot's exposure.
const EXPOSURE_POINTS: usize = 160;

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
    /// The spot must be out of this point's sight (no clear line on the bullet mesh): will he see me there, so will he
    /// fire. Ambush spots, the way point of a flank.
    pub unseen_by: Option<Vec3>,
    /// The spot must be out of the fire of this point: no line a bullet of [`Self::cover`] reaches the spot by
    /// (crates, cars and thin walls are no cover). Hide spots, heal cover.
    pub out_of_fire_of: Option<Vec3>,
    /// The bullet [`Self::out_of_fire_of`] means (the bot's `Tuning::cover.rule()`); none: every surface is cover.
    pub cover: Option<CoverRule>,
    /// Prefer spots higher than `around` (by this weight per metre, up to
    /// [`HIGH_CREDIT_M`] of height).
    pub prefer_high: f32,
    /// Also look at the floors of the storeys above the one the ring is
    /// on, up to this many metres over `around` (0: only the floor the
    /// grid's [`WalkGrid::locate`] finds, which lies at most 2 m above it).
    /// Only with a `sight_to`: a spot up there must see the point.
    pub up_m: f32,
    /// Also look at the floors of the storeys under the ring's, down to this many metres under `around` (0: none).
    /// Only with a `sight_to`, as `up_m`.
    pub down_m: f32,
    /// The bot's map side: the other side's spawn area is out.
    pub side: Option<i32>,
    /// Candidates sampled before the rays.
    pub samples: usize,
    /// Prefer spots the enemy's way does not see from many bearings ([`Exposure`]); none: the order is by height and nearness.
    pub exposure: Option<Exposure>,
}

impl SpotQuery {
    pub fn ring(around: Vec3, min_m: f32, max_m: f32) -> Self {
        Self {
            around,
            min_m,
            max_m,
            sight_to: None,
            sight_body: None,
            unseen_by: None,
            out_of_fire_of: None,
            cover: None,
            prefer_high: 0.0,
            up_m: 0.0,
            down_m: 0.0,
            side: None,
            samples: 48,
            exposure: None,
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
            .map(|node| u32::from(hazards.set_of(node).baked()) << SPAWN_BITS)
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
            progress: std::sync::OnceLock::new(),
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
                    progress: std::sync::OnceLock::new(),
                })
            },
            Self::digest,
        )
    }

    /// The progress of each region, worked out at the first call: two floods
    /// of the walk grid, once for the map (shared by its matches).
    fn progress(&self) -> &progress::Progress {
        self.progress.get_or_init(|| {
            progress::Progress::build(&self.world.grid, &self.regions, &self.world.map.spawns)
        })
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
            perches: Vec::new(),
            perch_share: None,
            past_fixed: Default::default(),
            walks: Default::default(),
            spot_lists: Default::default(),
            walk_ups: Default::default(),
            exit_field: Default::default(),
            escapes: Default::default(),
            escape: super::tuning::EscapeRule::UnderStartZone,
            foliage: Default::default(),
            grass: Default::default(),
            clear_step_sees_lip: false,
            cover_tally: CoverTally::default(),
            gang_tally: Default::default(),
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

    /// The humans' perches of this map (and their share of time), each put on
    /// the floor under it within a storey of its height; one with none is
    /// dropped, and one the planner cannot get to from the spawns of both
    /// sides (a floor the owner reached, 2026-10-26, that no walk of the
    /// planner leads to) is dropped here, once, not tried at every pick.
    pub fn set_perches(&mut self, perches: Vec<super::routes::Perch>, share: Option<super::routes::PerchShare>) {
        let world = self.world.clone();
        let grid = &world.grid;
        let teams: std::collections::BTreeSet<i32> = world.map.spawns.iter().map(|s| s.team).collect();
        let reach: Vec<Vec<u64>> = if perches.is_empty() {
            Vec::new()
        } else {
            teams
                .iter()
                .map(|&team| walk_from_spawns(&world, |s| s.team == team))
                .collect()
        };
        self.perches = perches
            .into_iter()
            .filter_map(|perch| {
                let node = grid.locate(perch.at, 1.5)?;
                let on = grid.position(node);
                let reached = |bits: &Vec<u64>| bits.get(node as usize / 64).is_some_and(|w| w >> (node % 64) & 1 == 1);
                ((on.y - perch.at.y).abs() < 1.5 && reach.iter().all(reached))
                    .then_some(super::routes::Perch { at: on, ..perch })
            })
            .collect();
        self.perch_share = share;
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

    /// What the walk cache has done so far.
    pub fn walk_stats(&self) -> WalkStats {
        let cache = self
            .walks
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        WalkStats {
            hits: cache.hits,
            misses: cache.misses,
            kept: cache.kept.len(),
            bytes: cache.kept.values().map(|w| w.len() * 4).sum(),
        }
    }

    /// The walk from floor `from` to floor `to` in metres, over the
    /// regions' directed links (a drop is short one way and long the
    /// other), times [`REGION_WALK_FACTOR`]: coarse (a region is a block of
    /// 32 cells), for telling a battery a storey up from one on the ground,
    /// not for a route. `None`: no way over the regions.
    pub fn walk_estimate(&self, from: NodeId, to: NodeId) -> Option<f32> {
        let regions = &self.base.regions;
        let goal = regions.of(to);
        let known = {
            let mut cache = self
                .walks
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let known = cache.kept.get(&goal).cloned();
            if known.is_some() {
                cache.hits += 1;
            } else {
                cache.misses += 1;
            }
            known
        };
        // Worked out with nothing locked, then kept.
        let walks = known.unwrap_or_else(|| {
            let made = Arc::new(region_walks_to(regions, goal));
            let mut cache = self
                .walks
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            cache.keep(goal, made.clone(), WALKS_MOST);
            made
        });
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

    /// How far forward `point` is for `side` by the walk: 0 at its own
    /// spawns, 1 at the enemy's, 0.5 where unknown ([`progress`]).
    pub fn progress_at(&self, point: Vec3, side: i32) -> f32 {
        let base = &self.base;
        base.progress().at(&base.world.grid, &base.regions, point, side)
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
        let zoned = self.hazards.marked_at(node);
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
    pub fn region_allowed_from(
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

    /// The regions of the floors as a shared lookup (the threat spots').
    pub fn region_lookup(&self) -> super::threat::RegionOf {
        let base = self.base.clone();
        std::sync::Arc::new(move |node| base.regions.of(node))
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
        self.route_nodes(start, goal, time, side, objective, &RouteFilter::open())
    }

    /// When the way from `from` to `to` opens, if only zones that switch shut it: the way a plain search keeps off no zone
    /// ([`path::plan_where`] over the spawn rule alone) is walked in 1 m steps, and each floor on it a zone shuts at `time`
    /// ([`Hazards::allowed`]) gives the soonest the schedule opens it ([`Hazards::gate_end`]); the latest of them is the
    /// answer. None when the way has no shut floor now (the failure is not a zone's), no way exists even with the zones
    /// open, or some floor on it is shut by a zone that does not switch, a live field, or a span the prediction does not
    /// end (no gate to wait for). It is one search over the whole grid: asked once by a bot that stands, not every tick.
    pub fn gate_opens(&self, from: Vec3, to: Vec3, time: u32, side: Option<i32>) -> Option<u32> {
        let grid = &self.world.grid;
        let (a, b) = (grid.locate(from, 1.5)?, grid.locate(to, LOCATE_M)?);
        let route = path::plan_where(grid, a, b, |node| self.spawn_ok(node, side, false))?;
        let mut opens = time;
        let mut prev = from;
        for waypoint in route.points() {
            let (here, there) = (prev, waypoint.at);
            let steps = (here.distance_xz(there).ceil() as u32).max(1);
            for step in 0..=steps {
                let t = step as f32 / steps as f32;
                let at = Vec3::new(
                    here.x + (there.x - here.x) * t,
                    here.y + (there.y - here.y) * t,
                    here.z + (there.z - here.z) * t,
                );
                if let Some(node) = grid.locate(at, 1.0) {
                    opens = opens.max(self.hazards.gate_end(node, time)?);
                }
            }
            prev = there;
        }
        (opens > time).then_some(opens)
    }

    /// [`Self::route_for`] between floors, keeping off what `filter` keeps
    /// out too (the spots a bot got stuck at, the landings of the jumps it
    /// gave up).
    pub fn route_nodes(
        &self,
        start: NodeId,
        goal: NodeId,
        time: u32,
        side: Option<i32>,
        objective: bool,
        filter: &RouteFilter,
    ) -> Option<Route> {
        let mut iters = usize::MAX;
        match self.start_route(start, goal, time, side, objective, filter, &mut iters) {
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
        filter: &RouteFilter,
        iters: &mut usize,
    ) -> Result<Option<Route>, path::Planning> {
        let grid = &self.world.grid;
        let at = grid.position(start);
        let weight = |node| filter.weight(grid, node);
        path::plan_start(
            grid,
            &self.base.regions,
            start,
            goal,
            |node| self.allowed_from(node, time, side, objective, at) && filter.floor_open(grid, node),
            |region| {
                self.region_allowed_from(region, time, side, objective, at)
                    && filter.region_open(region)
            },
            filter.max_jump_rise(),
            filter.weighs().then_some(&weight as &dyn Fn(NodeId) -> f32),
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
        filter: &RouteFilter,
    ) -> Option<Route> {
        let grid = &self.world.grid;
        let at = grid.position(start);
        planning.partial(grid, |node| {
            self.allowed_from(node, time, side, objective, at) && filter.floor_open(grid, node)
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
        filter: &RouteFilter,
        iters: &mut usize,
    ) -> Option<Option<Route>> {
        let grid = &self.world.grid;
        let at = grid.position(start);
        let weight = |node| filter.weight(grid, node);
        planning.resume(
            grid,
            &self.base.regions,
            |node| self.allowed_from(node, time, side, objective, at) && filter.floor_open(grid, node),
            |region| {
                self.region_allowed_from(region, time, side, objective, at)
                    && filter.region_open(region)
            },
            &weight,
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
            None,
            None,
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
            &|_| 1.0,
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
    /// and every floor next to it too (clear of the zone's edge). No route
    /// to any goal starts there, so without it the body would stand in the
    /// fire.
    ///
    /// The search never goes down into a zone stronger than the body's own
    /// (`Tactics::escape`): the floors open to it are those no zone
    /// marks, those of the start's own zones, those it may walk on, and
    /// those whose hit a second is not above the start's. From a mild zone
    /// the nearest way out may be over a strong one beside it (the 0.90
    /// stretch of the zone-damage bench); this goes round it, a few metres
    /// more. When that finds no way (nested zones, deep in a big zone) the
    /// ceiling is lifted and the nearest way over any floors is taken, as
    /// it was; so is the route to the way out when none stays under it.
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
    ///
    /// `filter`: its hard part (the landings of jumps and drops the body gave up,
    /// [`super::stuck::Stuck::shut_landings`]): neither the search nor the
    /// route to the way out goes over what it shuts.
    pub fn escape(
        &self,
        from: NodeId,
        goal: Option<Vec3>,
        time: u32,
        side: Option<i32>,
        objective: bool,
        filter: &RouteFilter,
    ) -> Option<Route> {
        let grid = &self.world.grid;
        let shut = |n: NodeId| !filter.hard_open(grid, n);
        let ok = |n: NodeId| self.allowed(n, time, side, objective);
        let (start_hit, start_set) = (
            self.hazards.hit_per_s(from).min(ESCAPE_HIT_CAP),
            self.hazards.set_of(from),
        );
        // Floors open to the search under a ceiling on a zone's hit.
        let open_under = |ceiling: f32| {
            move |n: NodeId| {
                self.hazards.set_of(n).is_none()
                    || self.hazards.set_of(n) == start_set
                    || ok(n)
                    || self.hazards.hit_per_s(n).min(ESCAPE_HIT_CAP) <= ceiling
            }
        };
        let start = grid.position(from);
        let search = |open: &dyn Fn(NodeId) -> bool| {
            let mut seen = std::collections::HashSet::from([from]);
            let mut queue = std::collections::VecDeque::from([(from, 0.0f32)]);
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
                    if grid.plans(a, b) && !shut(b) && open(b) && seen.insert(b) {
                        queue.push_back((b, d + grid.position(b).distance_xz(at)));
                    }
                }
            }
            outs
        };
        let mut ceiling = match self.escape {
            super::tuning::EscapeRule::UnderStartZone => start_hit,
            super::tuning::EscapeRule::Nearest => f32::INFINITY,
        };
        let mut outs = search(&open_under(ceiling));
        if outs.is_empty() && ceiling.is_finite() {
            ceiling = f32::INFINITY;
            outs = search(&open_under(ceiling));
        }
        let nearest = outs.iter().copied().min_by(|a, b| a.1.total_cmp(&b.1));
        // Out of a spawn area (no zone there): the nearest way, as ever.
        let goal = goal.filter(|_| self.hazards.marked_at(from));
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
        let open = open_under(ceiling);
        let route = path::plan_where(grid, from, out, |n| !shut(n) && open(n))
            .or_else(|| path::plan_where(grid, from, out, |n| !shut(n)));
        #[cfg(test)]
        if let Some(route) = &route {
            self.count_escape_route(route, from, time, side, objective);
        }
        route
    }

    /// [`grenade_avoid`](super::tuning::AvoidTuning) The floor a body at
    /// `from` steps to, to be out of the blast of a grenade at `rest`: the
    /// nearest floor it may stand on whose chest is `radius` or more from
    /// `rest`, by the way over the floors it may walk (what
    /// [`Self::allowed`] and `filter` let through; a link the planner does
    /// not use is not walked), within `reach_m` of walking. With none in
    /// reach, the floor in reach farthest from `rest`, if that is
    /// [`STEP_OUT_GAIN_M`] farther than where the body stands (it goes as far
    /// as it gets, and stays when it is at the best of it). From a floor
    /// already outside, the nearest floor outside is its own: it stays.
    ///
    /// One breadth-first search like [`Self::escape`]'s, over the floors
    /// within the reach only: about 10,000 at most, once for each body that
    /// steps out of each grenade.
    #[allow(clippy::too_many_arguments)]
    pub fn step_out(
        &self,
        from: NodeId,
        rest: Vec3,
        radius: f32,
        reach_m: f32,
        time: u32,
        side: Option<i32>,
        objective: bool,
        filter: &RouteFilter,
    ) -> Option<Vec3> {
        self.step_out_where(from, rest, radius, reach_m, time, side, objective, filter, false)
    }

    /// [`Self::step_out`], with `never_nearer`: a floor nearer `rest` than where the body stands (by more than
    /// [`STEP_OUT_GAIN_M`]) is not walked over, so the way found never leads toward the blast and back out of it.
    #[allow(clippy::too_many_arguments)]
    pub fn step_out_where(
        &self,
        from: NodeId,
        rest: Vec3,
        radius: f32,
        reach_m: f32,
        time: u32,
        side: Option<i32>,
        objective: bool,
        filter: &RouteFilter,
        never_nearer: bool,
    ) -> Option<Vec3> {
        let grid = &self.world.grid;
        let scan = self.step_out_scan(from, rest, radius, reach_m, time, side, objective, filter, 1, never_nearer);
        match scan.outside.first() {
            Some(&node) => Some(grid.position(node)),
            None => scan.farthest.filter(|&(_, far)| far > scan.start_far + STEP_OUT_GAIN_M).map(|(node, _)| grid.position(node)),
        }
    }

    /// [`Self::step_out`] checked by the route the body will walk (register #30): the floors outside `radius`, nearest by link
    /// distance first (at most [`STEP_OUT_CANDIDATES`] of them, so the planner runs on few), the first whose planned route
    /// ([`Self::route_nodes`], the walker's own) is within `reach_m` of walking; else the farthest floor within `far_m` (the
    /// most the time could allow, at the sprint), as [`Self::step_out`] gives it: a body that cannot get out of the blast still
    /// goes as far from it as it gets, the damage falling with the distance. The link distance is a lower bound of a walk, not the walk: a floor a drop below, round a
    /// wall or by stairs is further than it counts.
    #[allow(clippy::too_many_arguments)]
    pub fn step_out_routes(
        &self,
        from: NodeId,
        rest: Vec3,
        radius: f32,
        reach_m: f32,
        far_m: f32,
        time: u32,
        side: Option<i32>,
        objective: bool,
        filter: &RouteFilter,
    ) -> Option<(Vec3, StepOutPick)> {
        let grid = &self.world.grid;
        let scan = self.step_out_scan(from, rest, radius, reach_m.max(far_m), time, side, objective, filter, STEP_OUT_CANDIDATES, false);
        for (k, &node) in scan.outside.iter().enumerate() {
            let fits = node == from
                || self
                    .route_nodes(from, node, time, side, objective, filter)
                    .is_some_and(|route| route.length() <= reach_m);
            if fits {
                return Some((grid.position(node), if k == 0 { StepOutPick::Nearest } else { StepOutPick::Later }));
            }
        }
        let farthest = scan.farthest.filter(|&(_, far)| far > scan.start_far + STEP_OUT_GAIN_M);
        farthest.or_else(|| scan.outside.first().map(|&node| (node, scan.start_far))).map(|(node, _)| (grid.position(node), StepOutPick::Farthest))
    }

    /// The breadth-first search of [`Self::step_out`] over the floors within `reach_m` of walking: the floors outside `radius`
    /// (chest to `rest`) it finds, in order, up to `want` of them, and the floor in reach farthest from `rest`.
    #[allow(clippy::too_many_arguments)]
    fn step_out_scan(
        &self,
        from: NodeId,
        rest: Vec3,
        radius: f32,
        reach_m: f32,
        time: u32,
        side: Option<i32>,
        objective: bool,
        filter: &RouteFilter,
        want: usize,
        never_nearer: bool,
    ) -> StepOutScan {
        use super::view::CHEST;
        let grid = &self.world.grid;
        let up = Vec3::new(0.0, CHEST, 0.0);
        let far = |n: NodeId| (grid.position(n) + up - rest).length();
        let nearest_allowed = if never_nearer { far(from) - STEP_OUT_GAIN_M } else { f32::NEG_INFINITY };
        let open = |n: NodeId| self.allowed(n, time, side, objective);
        let mut seen = std::collections::HashSet::from([from]);
        let mut queue = std::collections::VecDeque::from([(from, 0.0f32)]);
        let mut scan = StepOutScan { outside: Vec::new(), farthest: None, start_far: far(from) };
        while let Some((a, d)) = queue.pop_front() {
            let (here, ok) = (far(a), open(a));
            if ok && here >= radius {
                scan.outside.push(a);
                if scan.outside.len() >= want {
                    break;
                }
            } else if ok && scan.farthest.is_none_or(|(_, best)| here > best) {
                scan.farthest = Some((a, here));
            }
            if seen.len() > STEP_OUT_FLOORS {
                break;
            }
            let at = grid.position(a);
            for (b, _) in grid.links(a) {
                if !grid.plans(a, b) || !filter.hard_open(grid, b) || seen.contains(&b) || far(b) < nearest_allowed {
                    continue;
                }
                let next = d + grid.position(b).distance_xz(at);
                if next <= reach_m {
                    seen.insert(b);
                    queue.push_back((b, next));
                }
            }
        }
        scan
    }

    /// The bench's counts of what a way out passes ([`escapes::counts`]): a
    /// zone stronger than the one it starts in (sampled every half metre
    /// along the route), and a drop that lands on a floor a zone marks.
    #[cfg(test)]
    fn count_escape_route(
        &self,
        route: &Route,
        from: NodeId,
        time: u32,
        side: Option<i32>,
        objective: bool,
    ) {
        let grid = &self.world.grid;
        let start_hit = self.hazards.hit_per_s(from).min(ESCAPE_HIT_CAP);
        let bad = |n: NodeId| {
            !self.hazards.set_of(n).is_none()
                && !self.allowed(n, time, side, objective)
                && self.hazards.hit_per_s(n).min(ESCAPE_HIT_CAP) > start_hit + 1e-6
        };
        let mut s = 0.0;
        let mut crossed = false;
        while s <= route.length() {
            crossed |= grid.locate(route.point_at(s), 1.0).is_some_and(bad);
            s += 0.5;
        }
        let dropped = route
            .points()
            .windows(2)
            .any(|w| w[0].at.y - w[1].at.y >= 1.0 && bad(w[1].node));
        escapes::count_route(crossed, dropped);
    }

    /// The floor no zone marks that the exit field leads to from `from`, a
    /// floor a zone marks: the field is a breadth-first search inward from
    /// the unmarked floors at the zones' edges over the marked ones, made
    /// once a match when first wanted (a few ms on the largest maps' tens
    /// of thousands of marked floors). For a body deeper in a zone than
    /// [`Self::escape`]'s search reaches (the Cologne bench's run 5: 25 m
    /// into a big zone, 73 escapes found nothing and it stood 37 s).
    pub fn exit_of(&self, from: NodeId) -> Option<NodeId> {
        // The field is made of the baked marks alone; a floor only a live field marks has its own way out.
        if self.hazards.set_of(from).baked() == 0 {
            return self.live_exit_of(from);
        }
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

    /// The floor no zone marks that a breadth-first walk from `from`, a floor only live fields mark (`Hazards::live`),
    /// first comes to over the marked floors, within [`LIVE_EXIT_FLOORS`] (the live fields come and go, so there is
    /// no field kept as for the baked zones). None when no live field lies on `from`.
    fn live_exit_of(&self, from: NodeId) -> Option<NodeId> {
        if self.hazards.live.set_of(from) == 0 {
            return None;
        }
        let grid = &self.world.grid;
        let mut seen = std::collections::HashSet::from([from]);
        let mut queue = std::collections::VecDeque::from([from]);
        while let Some(a) = queue.pop_front() {
            for (b, _) in grid.links(a) {
                if !grid.plans(a, b) || !seen.insert(b) {
                    continue;
                }
                if self.hazards.set_of(b).is_none() {
                    return Some(b);
                }
                if seen.len() < LIVE_EXIT_FLOORS {
                    queue.push_back(b);
                }
            }
        }
        None
    }

    /// [`Self::exit_of`]'s field: each marked floor's way out.
    fn exit_field(&self) -> std::collections::HashMap<NodeId, NodeId> {
        let grid = &self.world.grid;
        let marked = |n: NodeId| self.hazards.set_of(n).baked() != 0;
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
                self.hazards.marked_at(node)
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

    /// For a body with its feet at `feet` on floors a zone refuses at `time` ([`Hazards::allowed`]: the refusal
    /// [`Self::clear_line_for`] refuses its lines by): how far its walk may test a line over the refused floors, metres,
    /// the way to the nearest floor the zones allow and [`ZONE_EXIT_SLACK_M`] more. None where it stands on allowed
    /// floors. Its route out runs over the refused floors, so a line from it may cross as much of them as that way is long
    /// ([`Self::clear_line_leaving`]), and no more.
    pub fn zone_exit_reach(&self, feet: Vec3, time: u32) -> Option<f32> {
        let grid = &self.world.grid;
        let here = grid.locate(feet, 1.5)?;
        if self.hazards.allowed(here, time) {
            return None;
        }
        let nearest = |radius: f32| {
            grid.floors_in([feet.x - radius, feet.z - radius, feet.x + radius, feet.z + radius])
                .filter(|&node| self.hazards.allowed(node, time) && (grid.position(node).y - feet.y).abs() <= ZONE_EXIT_STOREY_M)
                .map(|node| grid.position(node).distance_xz(feet))
                .min_by(f32::total_cmp)
        };
        let depth = [2.0, 4.0, 8.0, 16.0, 32.0, ZONE_EXIT_MAX_M].into_iter().find_map(nearest).unwrap_or(ZONE_EXIT_MAX_M);
        Some(depth + ZONE_EXIT_SLACK_M)
    }

    /// [`Self::clear_line_for`] for a body on refused floors ([`Self::zone_exit_reach`] `reach`): the refused floors of
    /// the line's first stretch are walked, within `reach` of its start and before the first allowed floor, the way
    /// [`Self::clear_step_for`] lets a body out of a lip's band: out, not across a zone's core or back in. The spawn
    /// rule still holds.
    pub fn clear_line_leaving(&self, a: Vec3, b: Vec3, time: u32, side: Option<i32>, objective: bool, reach: f32) -> bool {
        let grid = &self.world.grid;
        let Some((from, to)) = line_ends(grid, a, b) else {
            return false;
        };
        let left = std::cell::Cell::new(false);
        path::straight_where(grid, from, to, |node| {
            if !self.spawn_ok(node, side, objective) {
                return false;
            }
            if self.hazards.allowed(node, time) {
                left.set(true);
                return true;
            }
            !left.get() && grid.position(node).distance_xz(a) <= reach
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
    ///
    /// With [`Self::clear_step_sees_lip`] the same holds for a lip's band
    /// ([`WalkGrid::near_lip`], which holds the lip floors too): the step
    /// does not end in it, and does not cross it unless the body starts in
    /// it (then it may step out of it, over the band's floors; one that
    /// stays in it, or goes along it, is refused). The band is a body's
    /// width past the lip, and a stride's overshoot ends over the lip
    /// (28 % of the unplanned falls on lips in the Cologne trace came from
    /// a step or weave in the last 0.5 s). Where no step is cleared the
    /// caller leaves the keys as they were (`drive_tick`: no weave, the
    /// body stands or walks straight).
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
        // The lip band: its end always, its floors on the line unless the
        // body starts in it (`near_lip` holds the lip floors themselves).
        let sees_lip = self.clear_step_sees_lip;
        let leaving = grid.near_lip(from);
        !(trap(to) || (sees_lip && grid.near_lip(to)))
            && path::straight_where(grid, from, to, |node| {
                self.allowed(node, time, side, objective)
                    && !trap(node)
                    && !(sees_lip && !leaving && grid.near_lip(node))
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
            let eye = still_eye(feet, true);
            let d = (at - eye).length();
            if d <= r && best.is_none_or(|(b, _)| d < b) && self.sees(eye, at) {
                best = Some((d, feet));
            }
        }
        best.map(|(_, feet)| feet)
    }

    /// Whether `b` is seen from `a` (points at the eyes).
    pub fn sees(&self, a: Vec3, b: Vec3) -> bool {
        super::rays::visible(&self.world.sight, a, b)
    }

    /// Whether a bullet fired at `b` from `a` (points at the eyes) is a threat to `b`: the danger tests' question,
    /// what [`Self::sees`] is to sight (will he fire at me: ask `sees`; can his bullets reach me: ask this). With a
    /// `cover` rule a surface that lets the bullet through is not cover (the client's rule,
    /// `RayTree::damage_share`): the line is open when the share of the damage that reaches `b` is over the rule's.
    /// Without one, as [`Self::sees`].
    pub fn shot_reaches(&self, cover: Option<CoverRule>, a: Vec3, b: Vec3) -> bool {
        self.line_of_fire(cover, a, b).open
    }

    /// [`Self::shot_reaches`] with the share of the damage that reaches `b` (1 for a clear line and 0 for a shut one
    /// when there is no rule).
    pub fn line_of_fire(&self, cover: Option<CoverRule>, a: Vec3, b: Vec3) -> Fire {
        self.fire_line(cover, a, b, false)
    }

    /// [`Self::line_of_fire`] with the flight followed to its end: the share of a shut line is what really reaches
    /// (the plain one gives up as soon as the share is under the rule's, so a shut line's share is only an upper bound).
    pub fn line_of_fire_exact(&self, cover: Option<CoverRule>, a: Vec3, b: Vec3) -> Fire {
        self.fire_line(cover, a, b, true)
    }

    fn fire_line(&self, cover: Option<CoverRule>, a: Vec3, b: Vec3, exact: bool) -> Fire {
        use std::sync::atomic::Ordering::Relaxed;
        let tally = &self.cover_tally;
        let Some(CoverRule { pierce, max_share, .. }) = cover else {
            tally.plain.fetch_add(1, Relaxed);
            let open = self.sees(a, b);
            return Fire { open, share: if open { 1.0 } else { 0.0 } };
        };
        let line = super::rays::follow(&self.world.sight, a, b, pierce, if exact { 0.0 } else { max_share });
        let share = line.share.value();
        if share > max_share {
            // Open. Through cover where the solid mesh has a wall: the lines the pierce opens.
            if self.sees(a, b) {
                tally.open_clear.fetch_add(1, Relaxed);
            } else {
                tally.open_through.fetch_add(1, Relaxed);
                tally.events_seen.fetch_add(1, Relaxed);
                // The first events of the match only (the count above has them all): a log is read by its notes.
                if tally.events_kept.load(Relaxed) < COVER_EVENTS as u64 {
                    tally.events_kept.fetch_add(1, Relaxed);
                    let mut events = tally.events.lock().unwrap_or_else(|e| e.into_inner());
                    events.push(CoverEvent { from: a, to: b, share, crossed: line.crossed });
                }
            }
            Fire { open: true, share }
        } else {
            if share > 0.0 {
                tally.shut_partial.fetch_add(1, Relaxed);
            } else {
                tally.shut_stopped.fetch_add(1, Relaxed);
            }
            Fire { open: false, share }
        }
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
        super::rays::ray_cast(&self.world.sight, a, dir, length - 0.01)
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

    /// Why [`Self::spots`] gives nothing for `query` (the bench's class; its own `rng`): the stage that lost the most
    /// of its samples. A probe with no floor near the anchor's height is split by whether a floor lies under or over
    /// it within 30 m (`floor_off_height`) or none at all (`no_floor`).
    #[cfg(test)]
    pub fn spots_why(&self, query: &SpotQuery, time: u32, rng: &mut Rng) -> &'static str {
        let grid = &self.world.grid;
        let mut fails: Vec<&'static str> = Vec::new();
        let mut by_walk: Option<Arc<WalkUp>> = None;
        for _ in 0..query.samples {
            let angle = rng.range(0.0, std::f32::consts::TAU);
            let r = rng.range(query.min_m, query.max_m);
            let probe = Vec3::new(query.around.x + r * angle.cos(), query.around.y, query.around.z + r * angle.sin());
            let mut nodes: Vec<NodeId> = grid.locate(Vec3::new(probe.x, probe.y + 1.0, probe.z), 2.0).into_iter().collect();
            if query.up_m > 0.0 && query.sight_to.is_some() {
                self.storeys_over(probe, query.up_m, &mut nodes);
            }
            if query.down_m > 0.0 && query.sight_to.is_some() {
                self.storeys_under(probe, query.down_m, &mut nodes);
            }
            if nodes.is_empty() {
                let off = grid.locate(Vec3::new(probe.x, probe.y + 1.0, probe.z), 30.0).is_some();
                fails.push(if off { "floor_off_height" } else { "no_floor" });
                continue;
            }
            for node in nodes {
                let at = grid.position(node);
                let d = at.distance_xz(query.around);
                fails.push(if d < query.min_m || d > query.max_m {
                    "off_ring"
                } else if !self.walkable(node) {
                    "not_walkable"
                } else if !self.allowed(node, time, query.side, false) {
                    "not_allowed"
                } else if at.y > query.around.y + STOREY_FROM_M
                    && !by_walk
                        .get_or_insert_with(|| self.walk_up_set(query.around, query.max_m + WALK_UP_MARGIN_M))
                        .contains(node)
                {
                    "storey_no_walk"
                } else if query.sight_to.is_some_and(|t| !self.sees(still_eye(at, true), t + Vec3::new(0.0, 1.0, 0.0))) {
                    "no_sight"
                } else {
                    "other_filter"
                });
            }
        }
        super::threat::most_failed(&fails)
    }

    /// The best spots for `query`, best first, at most `n`.
    pub fn spots(&self, query: &SpotQuery, time: u32, n: usize, rng: &mut Rng) -> Vec<Spot> {
        #[cfg(test)]
        super::bench_work::add(super::bench_work::Work::Spots, 1);
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
            // The floor the grid finds near the probe, and with `up_m` the
            // floors of the storeys over it.
            let mut nodes: Vec<NodeId> = grid
                .locate(Vec3::new(probe.x, probe.y + 1.0, probe.z), 2.0)
                .into_iter()
                .collect();
            if query.up_m > 0.0 && query.sight_to.is_some() {
                self.storeys_over(probe, query.up_m, &mut nodes);
            }
            if query.down_m > 0.0 && query.sight_to.is_some() {
                self.storeys_under(probe, query.down_m, &mut nodes);
            }
            for node in nodes {
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
                let rise = (at.y - query.around.y).min(HIGH_CREDIT_M);
                let score = query.prefer_high * rise - d * 0.01;
                found.push(Spot { at, node, score });
            }
        }
        found.sort_by(|a, b| b.score.total_cmp(&a.score));
        // Rays lazily, from the best down.
        let mut out = Vec::new();
        let mut by_walk: Option<Arc<WalkUp>> = None;
        // A hiding spot is weighed by the share of his bullet that reaches it, not only filtered: every candidate is
        // tried, the first to pass in the base order is what the filter alone gives.
        let mut shares: Vec<f32> = Vec::new();
        let cost = query.cover.and_then(|rule| rule.spot_cost).filter(|_| query.out_of_fire_of.is_some());
        // The enemy's route points the exposure is cast to, when the query asks for it and the map has them.
        let approach: Vec<Vec3> = match (query.exposure, query.side) {
            (Some(_), Some(side)) => {
                let enemy = self.world.map.spawns.iter().map(|s| s.team).find(|team| *team != side);
                enemy.map_or_else(Vec::new, |enemy| self.routes_of(enemy).flat_map(|r| r.points.iter().copied()).collect())
            }
            _ => Vec::new(),
        };
        let weighing = query.exposure.filter(|_| !approach.is_empty());
        // The bearings of each spot in `out`, when weighed.
        let mut seen_from: Vec<u8> = Vec::new();
        for mut spot in found {
            if out.len() >= n && cost.is_none() && weighing.is_none() {
                break;
            }
            // A spot on a storey over the anchor is one a walk up gets to (stairs,
            // a ramp), not one only a jump chain does ([`Self::walk_up_from`]).
            if spot.at.y > query.around.y + STOREY_FROM_M
                && !by_walk
                    .get_or_insert_with(|| self.walk_up_set(query.around, query.max_m + WALK_UP_MARGIN_M))
                    .contains(spot.node)
            {
                continue;
            }
            let eye = still_eye(spot.at, true);
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
            if let Some(from) = query.unseen_by
                && self.sees(still_eye(from, false), still_eye(spot.at, false))
            {
                continue;
            }
            if let Some(from) = query.out_of_fire_of {
                let (from, to) = (still_eye(from, false), still_eye(spot.at, false));
                match cost {
                    Some(cost) => {
                        let fire = self.line_of_fire_exact(query.cover, from, to);
                        if fire.open {
                            continue;
                        }
                        spot.score -= cost.get() * fire.share;
                        shares.push(fire.share);
                    }
                    None if self.shot_reaches(query.cover, from, to) => continue,
                    None => {}
                }
            }
            if let Some(exposure) = &weighing {
                let bearings = self.spot_exposure(spot.at, query.around, &approach, exposure);
                spot.score -= exposure.weight.get() * f32::from(bearings);
                seen_from.push(bearings);
            }
            out.push(spot);
        }
        if weighing.is_some() && cost.is_none() && !out.is_empty() {
            use std::sync::atomic::Ordering::Relaxed;
            // `seen_from[i]` is `out[i]`'s; `out[0]` is what height and nearness alone give first.
            let mut ranked: Vec<(Spot, u8)> = out.into_iter().zip(seen_from).collect();
            let first = ranked[0];
            ranked.sort_by(|a, b| b.0.score.total_cmp(&a.0.score));
            let tally = &self.cover_tally;
            tally.exposure_lists.fetch_add(1, Relaxed);
            tally.exposure_changed.fetch_add(u64::from(ranked[0].0.node != first.0.node), Relaxed);
            tally.exposure_other_filter.fetch_add(u64::from(first.1), Relaxed);
            tally.exposure_other_weighed.fetch_add(u64::from(ranked[0].1), Relaxed);
            ranked.truncate(n);
            out = ranked.into_iter().map(|(spot, _)| spot).collect();
        }
        if cost.is_some() && !out.is_empty() {
            use std::sync::atomic::Ordering::Relaxed;
            // `shares[i]` is `out[i]`'s; the filter alone gives out[0]. Best first by the weighed score.
            let tally = &self.cover_tally;
            let mut ranked: Vec<(Spot, f32)> = out.into_iter().zip(shares).collect();
            let filter_first = ranked[0];
            ranked.sort_by(|a, b| b.0.score.total_cmp(&a.0.score));
            tally.spot_weighed.fetch_add(1, Relaxed);
            if ranked[0].0.node != filter_first.0.node
                && tally.spot_changed.fetch_add(1, Relaxed) < SPOT_EVENTS as u64
                && let Some(from) = query.out_of_fire_of
            {
                let mut events = tally.spot_events.lock().unwrap_or_else(|e| e.into_inner());
                events.push(SpotEvent {
                    around: query.around,
                    from,
                    filter: (filter_first.0.at, filter_first.1),
                    weighed: (ranked[0].0.at, ranked[0].1),
                });
            }
            tally.spot_share_filter_milli.fetch_add((filter_first.1 * 1000.0) as u64, Relaxed);
            tally.spot_share_weighed_milli.fetch_add((ranked[0].1 * 1000.0) as u64, Relaxed);
            ranked.truncate(n);
            out = ranked.into_iter().map(|(spot, _)| spot).collect();
        }
        out
    }

    /// [A195] The sectors (30 degrees) of `approach` seen from `spot`, not counting those within `exposure.anchor_deg` of the
    /// bearing to `anchor`: a clear line from the spot's crouched eye to the standing eye of a point `near_m` to `far_m` off.
    /// At most [`EXPOSURE_POINTS`] points are cast to (every k-th of those in range).
    pub fn spot_exposure(&self, spot: Vec3, anchor: Vec3, approach: &[Vec3], exposure: &Exposure) -> u8 {
        const SECTORS: usize = 12;
        let eye = still_eye(spot, true);
        let in_range: Vec<Vec3> = approach
            .iter()
            .copied()
            .filter(|p| (exposure.near_m..=exposure.far_m).contains(&p.distance_xz(spot)))
            .collect();
        let stride = in_range.len().div_ceil(EXPOSURE_POINTS).max(1);
        let mut seen = [false; SECTORS];
        for p in in_range.iter().step_by(stride) {
            if self.sees(eye, still_eye(*p, false)) {
                let angle = (p.z - spot.z).atan2(p.x - spot.x).to_degrees().rem_euclid(360.0);
                seen[((angle / (360.0 / SECTORS as f32)) as usize).min(SECTORS - 1)] = true;
            }
        }
        let anchor_deg = (anchor.z - spot.z).atan2(anchor.x - spot.x).to_degrees().rem_euclid(360.0);
        seen.iter()
            .enumerate()
            .filter(|&(i, &seen)| {
                let centre = (i as f32 + 0.5) * (360.0 / SECTORS as f32);
                let off = (centre - anchor_deg).rem_euclid(360.0);
                seen && off.min(360.0 - off) > exposure.anchor_deg
            })
            .count() as u8
    }

    /// The floors within `radius` (on the ground plane) of `around` that a
    /// body gets to from the floor at `around` by walking links only (no
    /// jump, no drop): stairs and ramps count, a ledge only a jump gets up
    /// does not.
    ///
    /// Root cause (`push-record.md` V-S2a smoke): the bots' jumps are wrong
    /// 89 % of the time and fail 40 % (the bench's wrong-jump detector, tip
    /// 9ae23278, 1,416 presses in 64 matches); a spot reached only by a jump
    /// chain sent bots to climbs the motor does badly (stuck +21 %, jump
    /// stand-turn +31 % in the smoke of the search without this rule). The
    /// jumps are the grid's and the motor's to mend; until they are, the
    /// storeys over the ring are the ones a walk reaches.
    ///
    /// The search is a breadth-first one in the order of the links, and its cap
    /// ([`WALK_UP_MAX_FLOORS`]) cuts it at the same floor whatever holds the
    /// set; the set is kept for the same point and radius (the grid does
    /// not change), so a list made again 15 s later does not search again.
    fn walk_up_set(&self, around: Vec3, radius: f32) -> Arc<WalkUp> {
        let key = [around.x.to_bits(), around.y.to_bits(), around.z.to_bits(), radius.to_bits()];
        let mut kept = self.walk_ups.lock().unwrap_or_else(|e| e.into_inner());
        if let Some((_, set)) = kept.iter().find(|(k, _)| *k == key) {
            return set.clone();
        }
        let set = Arc::new(self.walk_up_from(around, radius));
        if kept.len() >= WALK_UP_KEPT {
            kept.remove(0);
        }
        kept.push((key, set.clone()));
        set
    }

    /// The search [`Self::walk_up_set`] keeps.
    fn walk_up_from(&self, around: Vec3, radius: f32) -> WalkUp {
        use mapdata::LinkKind;
        let grid = &self.world.grid;
        let mut seen = WalkUp::new(grid.len());
        let Some(start) = grid.locate(around, LOCATE_M) else {
            return seen;
        };
        let mut queue = vec![start];
        let mut count = 1usize;
        seen.insert(start);
        let mut head = 0;
        while head < queue.len() {
            let u = queue[head];
            head += 1;
            if count >= WALK_UP_MAX_FLOORS {
                break;
            }
            for (v, kind) in grid.links(u) {
                if kind == LinkKind::Walk
                    && !seen.contains(v)
                    && grid.position(v).distance_xz(around) <= radius
                {
                    seen.insert(v);
                    count += 1;
                    queue.push(v);
                }
            }
        }
        seen
    }

    /// The set [`Self::walk_up_from`] was before the bit set and the kept
    /// sets: the reference the test holds the new one to.
    #[cfg(test)]
    pub(super) fn walk_up_reference(&self, around: Vec3, radius: f32) -> std::collections::HashSet<NodeId> {
        use mapdata::LinkKind;
        let grid = &self.world.grid;
        let mut seen = std::collections::HashSet::new();
        let Some(start) = grid.locate(around, LOCATE_M) else {
            return seen;
        };
        let mut queue = std::collections::VecDeque::from([start]);
        seen.insert(start);
        while let Some(u) = queue.pop_front() {
            if seen.len() >= WALK_UP_MAX_FLOORS {
                break;
            }
            for (v, kind) in grid.links(u) {
                if kind == LinkKind::Walk
                    && !seen.contains(&v)
                    && grid.position(v).distance_xz(around) <= radius
                {
                    seen.insert(v);
                    queue.push_back(v);
                }
            }
        }
        seen
    }

    /// Adds to `nodes` the floors over `probe` that the grid's `locate` does
    /// not reach (more than 2 m over `probe.y`, up to `up_m` over it): the
    /// nearest of each [`STOREY_BAND_M`] band within [`STOREY_RADIUS_M`] of
    /// the probe, one that a walk gets to and no pocket holds. Read from the
    /// grid's floors of the cells (no ray).
    fn storeys_over(&self, probe: Vec3, up_m: f32, nodes: &mut Vec<NodeId>) {
        self.storeys_in(probe, probe.y + 2.0, probe.y + up_m, nodes);
    }

    /// The floors of the storeys under the probe's, down to `down_m` under it: as [`Self::storeys_over`] (the
    /// nearest fine floor a band), under the floor the grid's `locate` finds at the probe (2 m round it).
    fn storeys_under(&self, probe: Vec3, down_m: f32, nodes: &mut Vec<NodeId>) {
        self.storeys_in(probe, probe.y - down_m, probe.y - 1.0, nodes);
    }

    /// The nearest fine floor (walkable, not in a pocket nor by a fall or a lip) of each storey band between `low`
    /// (out) and `high` (in) round the probe.
    fn storeys_in(&self, probe: Vec3, low: f32, high: f32, nodes: &mut Vec<NodeId>) {
        let grid = &self.world.grid;
        let r = STOREY_RADIUS_M;
        let mut best: Vec<(usize, NodeId, f32)> = Vec::new();
        for node in grid.floors_in([probe.x - r, probe.z - r, probe.x + r, probe.z + r]) {
            let at = grid.position(node);
            if at.y <= low || at.y > high || nodes.contains(&node) {
                continue;
            }
            let d = at.distance_xz(probe);
            // Not at an edge: a hold on a ledge strafes and turns in a fight and
            // steps off it (the smoke of the search over every storey: falls off
            // +16 % in all, +75 % on Cologne's decks).
            if d > r
                || !self.walkable(node)
                || grid.in_pocket(node)
                || grid.near_fall(node)
                || grid.near_lip(node)
            {
                continue;
            }
            let band = ((at.y - low) / STOREY_BAND_M) as usize;
            match best.iter_mut().find(|b| b.0 == band) {
                Some(b) if d < b.2 => *b = (band, node, d),
                Some(_) => {}
                None => best.push((band, node, d)),
            }
        }
        best.sort_by_key(|b| b.0);
        nodes.extend(best.into_iter().map(|b| b.1));
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
    walk_from_spawns(world, |_| true)
}

/// The floors a walk from the floors of the spawns `of` gets to
/// ([`walkable_from_spawns`] for all of them).
fn walk_from_spawns(world: &World, of: impl Fn(&mapdata::Spawn) -> bool) -> Vec<u64> {
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
    for spawn in world.map.spawns.iter().filter(|s| of(s)) {
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

    static COUNTS: [AtomicU64; 5] = [const { AtomicU64::new(0) }; 5];

    /// A way out passed a stronger zone than its start's, or dropped onto a
    /// floor a zone marks.
    pub fn count_route(crossed: bool, dropped: bool) {
        COUNTS[3].fetch_add(u64::from(crossed), Ordering::Relaxed);
        COUNTS[4].fetch_add(u64::from(dropped), Ordering::Relaxed);
    }

    pub fn count(found: bool, on: bool) {
        let k = if !found { 2 } else { usize::from(on) };
        COUNTS[k].fetch_add(1, Ordering::Relaxed);
    }

    /// (back the nearest way, on toward the goal, none in the search, over
    /// a stronger zone, a drop onto a zone's floor).
    pub fn counts() -> [u64; 5] {
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

    /// The set of floors a walk up gets to is the hash set's, floor for
    /// floor, for points across the grid and radii (the spot lists made
    /// from it are then the same), and the kept set is the one made.
    #[test]
    fn the_walk_up_set_is_the_reference_set_floor_for_floor() {
        let t = Tactics::new(tests::platform_world(), 1.0);
        let grid = &t.world.grid;
        let mut rng = Rng::new(7);
        let mut nonempty = 0;
        for i in 0..300 {
            let around = Vec3::new(rng.range(0.0, 70.0), rng.range(-1.0, 6.0), rng.range(0.0, 45.0));
            let radius = [4.0, 12.0, 30.0, 80.0][i % 4];
            let reference = t.walk_up_reference(around, radius);
            let set = t.walk_up_set(around, radius);
            nonempty += usize::from(!reference.is_empty());
            for node in 0..grid.len() as NodeId {
                assert_eq!(set.contains(node), reference.contains(&node), "{around:?} {radius} {node}");
            }
            assert!(Arc::ptr_eq(&set, &t.walk_up_set(around, radius)));
        }
        assert!(nonempty > 20, "{nonempty}");
    }

    /// The same on the real maps, where the search is cut by its cap too.
    /// Run with `SRP_MAPDATA_DIR` set, in release.
    #[test]
    #[ignore = "needs the private map files"]
    fn the_walk_up_set_is_the_reference_set_on_the_real_maps() {
        let Some(dir) = std::env::var_os("SRP_MAPDATA_DIR") else {
            panic!("set SRP_MAPDATA_DIR");
        };
        let (mut cut, mut checked) = (0, 0);
        let (mut old_t, mut new_t, mut kept_t) = (std::time::Duration::ZERO, std::time::Duration::ZERO, std::time::Duration::ZERO);
        for level in 1..=8u8 {
            for mode in [2u8, 4] {
                let path = std::path::Path::new(&dir).join(format!("level_{level:02}_day_{mode}.mapdata"));
                let map = mapdata::MapData::read(&path).unwrap();
                let world = Arc::new(World::build(map, mapdata::CELL, mapdata::Body::default()));
                let t = Tactics::new(world, 1.0);
                let grid = &t.world.grid;
                let mut rng = Rng::new(u64::from(level) * 10 + u64::from(mode));
                for i in 0..25 {
                    let Some(spawn) = t.world.map.spawns.get(i % t.world.map.spawns.len().max(1)) else {
                        break;
                    };
                    let around = spawn.position
                        + Vec3::new(rng.range(-60.0, 60.0), 0.0, rng.range(-60.0, 60.0));
                    let radius = [30.0, 45.0, 65.0][i % 3];
                    let start = std::time::Instant::now();
                    let reference = t.walk_up_reference(around, radius);
                    old_t += start.elapsed();
                    let start = std::time::Instant::now();
                    let set = t.walk_up_set(around, radius);
                    new_t += start.elapsed();
                    let start = std::time::Instant::now();
                    let _ = t.walk_up_set(around, radius);
                    kept_t += start.elapsed();
                    cut += usize::from(reference.len() >= WALK_UP_MAX_FLOORS);
                    checked += 1;
                    let in_set = (0..grid.len() as NodeId).filter(|&n| set.contains(n)).count();
                    assert_eq!(in_set, reference.len(), "{level} {mode} {around:?} {radius}");
                    assert!(reference.iter().all(|&n| set.contains(n)));
                }
            }
        }
        eprintln!("walk-up sets checked {checked}, cut by the cap {cut}");
        eprintln!(
            "per set: old {:?}, new (made) {:?}, kept {:?}",
            old_t / checked as u32,
            new_t / checked as u32,
            kept_t / checked as u32
        );
    }

    /// The progress table is made at the first ask, not with the base: a map
    /// nothing asks it of (every switch off) never pays for its floods.
    #[test]
    fn the_progress_table_is_made_at_the_first_ask() {
        let tactics = Tactics::new(field(), 1.0);
        assert!(tactics.base.progress.get().is_none());
        let a = tactics.progress_at(Vec3::new(5.0, 0.0, 10.0), 0);
        assert!(tactics.base.progress.get().is_some());
        assert_eq!(a, tactics.progress_at(Vec3::new(5.0, 0.0, 10.0), 0));
    }

    /// What the progress table (R2: two grid floods) costs next to the whole
    /// base, per offered map. Run with `SRP_MAPDATA_DIR` set, in release:
    /// `cargo test -p match-server --release progress_cost -- --ignored --nocapture`.
    #[test]
    #[ignore = "a measurement; needs the private map files"]
    fn progress_cost_on_the_real_maps() {
        let Some(dir) = std::env::var_os("SRP_MAPDATA_DIR") else {
            panic!("set SRP_MAPDATA_DIR");
        };
        for level in 1..=8u8 {
            for mode in [2u8, 4] {
                let path = std::path::Path::new(&dir).join(format!("level_{level:02}_day_{mode}.mapdata"));
                let map = mapdata::MapData::read(&path).unwrap();
                let world = Arc::new(World::build(map, mapdata::CELL, mapdata::Body::default()));
                let start = std::time::Instant::now();
                let base = TacticsBase::build(world.clone(), 1.0);
                let whole = start.elapsed();
                let mut each = Vec::new();
                for _ in 0..5 {
                    let start = std::time::Instant::now();
                    let again = progress::Progress::build(&world.grid, &base.regions, &world.map.spawns);
                    each.push(start.elapsed());
                    assert_eq!(&again, base.progress());
                }
                each.sort();
                let known = (0..base.regions.len() as u32)
                    .filter(|&r| base.progress().of_region(r, 0) != progress::UNKNOWN)
                    .count();
                if known == 0 {
                    for sp in &world.map.spawns {
                        eprintln!(
                            "  no numbers, spawn team {} at {:?}: locate 1.5 {:?}, 3 {:?}, 8 {:?}",
                            sp.team,
                            sp.position,
                            world.grid.locate(sp.position, 1.5),
                            world.grid.locate(sp.position, 3.0),
                            world.grid.locate(sp.position, 8.0)
                        );
                    }
                }
                let teams: std::collections::BTreeSet<i32> = world.map.spawns.iter().map(|s| s.team).collect();
                eprintln!(
                    "progress level {level} day {mode}: teams {teams:?} known regions {known} floors {} regions {} progress median {:?} min {:?} max {:?}, whole base {:?}",
                    world.grid.len(),
                    base.regions.len(),
                    each[2],
                    each[0],
                    each[4],
                    whole
                );
            }
        }
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
        assert_eq!(TACTICS_ALGO, 4);
    }

    const PINNED_FIELD: u64 = 13_582_078_593_300_033_124;

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

    /// A slab 2 m over the field, its edges no wall (a lip: a drop the body
    /// makes, no fall): the lip's band is the floors a body's width in from
    /// its east edge.
    fn lip_slab() -> Arc<World> {
        field_with_solids(&floor(50.0, 30.0, 56.0, 36.0, 2.0), |_| {})
    }

    /// The side step ends on a floor in a lip's band, and a stride there goes
    /// over the lip: it is refused (the other side is clear), the line along
    /// the band too; a body already in the band steps out of it over the
    /// band's floors. With `clear_step_sees_lip` off every one is clear, as
    /// it was.
    #[test]
    fn no_side_step_onto_a_lips_band() {
        let world = lip_slab();
        let grid = &world.grid;
        let mut tactics = Tactics::new(world.clone(), 1.0);
        let at = |x: f32, z: f32| Vec3::new(x, 2.0, z);
        let floor_at = |x: f32, z: f32| grid.locate(at(x, z), 0.3).unwrap();
        let (inside, band) = (floor_at(54.2, 33.0), floor_at(55.7, 33.0));
        assert!(grid.is_lip(floor_at(55.9, 33.0)));
        assert!(grid.near_lip(band) && !grid.near_lip(inside));
        assert!(!grid.near_fall(band), "a lip, not a fall");
        let step =
            |tactics: &Tactics, a: Vec3, b: Vec3| tactics.clear_step_for(a, b, 0, Some(0), false);
        // OFF by default (the merge switched off): the old step.
        assert!(!tactics.clear_step_sees_lip);
        tactics.clear_step_sees_lip = true;
        // Toward the lip: the end is in the band; away from it: clear.
        assert!(!step(&tactics, at(54.2, 33.0), at(55.7, 33.0)));
        assert!(step(&tactics, at(54.2, 33.0), at(52.7, 33.0)));
        // In the band: out of it (over its floors) is clear, along it or
        // further in is not.
        assert!(step(&tactics, at(55.7, 33.0), at(54.2, 33.0)));
        assert!(!step(&tactics, at(55.7, 33.0), at(55.7, 34.5)));
        assert!(!step(&tactics, at(55.7, 33.0), at(55.7, 31.5)));
        tactics.clear_step_sees_lip = false;
        assert!(step(&tactics, at(54.2, 33.0), at(55.7, 33.0)));
        assert!(step(&tactics, at(54.2, 33.0), at(52.7, 33.0)));
        assert!(step(&tactics, at(55.7, 33.0), at(54.2, 33.0)));
        assert!(step(&tactics, at(55.7, 33.0), at(55.7, 34.5)));
    }

    /// With the switch off the step is the old one exactly (falls and
    /// pockets, the end floor and the line); on, it only refuses more: from
    /// every floor of the slab and the field around it, in eight ways.
    #[test]
    fn the_lip_switch_off_is_the_old_step_and_on_refuses_more() {
        let world = lip_slab();
        let grid = &world.grid;
        let mut on = Tactics::new(world.clone(), 1.0);
        on.clear_step_sees_lip = true;
        let off = Tactics::new(world.clone(), 1.0);
        let old = |a: Vec3, b: Vec3| {
            let (Some(from), Some(to)) = (grid.locate(a, 1.0), grid.locate(b, 0.6)) else {
                return false;
            };
            let home = grid.pocket_of(from);
            let trap = |node: NodeId| {
                grid.near_fall(node)
                    || grid.pocket_of(node).is_some_and(|pocket| {
                        Some(pocket) != home
                            && !crate::v026g::bots::escape::standing_exit(
                                grid,
                                &off.escapes,
                                pocket,
                            )
                    })
            };
            !trap(to)
                && path::straight_where(grid, from, to, |node| {
                    off.allowed(node, 0, Some(0), false) && !trap(node)
                })
        };
        let (mut asked, mut refused, mut cleared) = (0, 0, 0);
        for node in grid.floors_in([46.0, 26.0, 60.0, 40.0]) {
            let a = grid.position(node);
            for d in 0..8 {
                let angle = d as f32 * std::f32::consts::FRAC_PI_4;
                let b = a + Vec3::new(angle.cos(), 0.0, angle.sin()) * 1.5;
                let before = off.clear_step_for(a, b, 0, Some(0), false);
                assert_eq!(before, old(a, b), "{a:?} to {b:?}");
                let after = on.clear_step_for(a, b, 0, Some(0), false);
                assert!(
                    before || !after,
                    "on cleared what off refused: {a:?} to {b:?}"
                );
                asked += 1;
                refused += usize::from(before && !after);
                cleared += usize::from(after);
            }
        }
        // The slab's edges refuse some, and plenty stay clear.
        assert!(
            refused > 20 && cleared > refused,
            "{refused} refused, {cleared} clear of {asked}"
        );
    }

    /// The slide in the world: a body facing north-west of the lip so that its left side step ends in the band
    /// is turned to its right (the side step `drive::choose_step` asks `clear_step_for` about, over the move of
    /// its keys); in the band, facing the lip, both sides run along it and both are refused, so it stands.
    #[test]
    fn a_side_step_toward_a_lip_slides_to_the_other_side() {
        use super::super::drive::{Step, choose_step, side_step};
        use crate::v026g::drive::key;
        use super::super::kin;
        let world = lip_slab();
        let mut tactics = Tactics::new(world.clone(), 1.0);
        tactics.clear_step_sees_lip = true;
        // Facing so that the body's left is +x, toward the slab's east lip at x = 56.
        let yaw = kin::yaw_of([1.0, 0.0]) - std::f32::consts::FRAC_PI_2;
        let [lx, lz] = kin::forward(yaw + std::f32::consts::FRAC_PI_2);
        assert!((lx - 1.0).abs() < 1e-4 && lz.abs() < 1e-4, "{lx} {lz}");
        let choose = |tactics: &Tactics, at: Vec3, keys: u32, strafe: u32, slides: bool| {
            let ask = |keys: u32, strafe: u32| {
                let to = at + side_step(keys, strafe, yaw, tactics.clear_step_sees_lip);
                tactics.clear_step_for(at, to, 0, Some(0), false)
            };
            choose_step(
                keys,
                strafe,
                false,
                tactics.clear_step_sees_lip,
                slides,
                false,
                ask,
            )
            .0
        };
        let from = Vec3::new(54.2, 2.0, 33.0);
        for walk in [0, key::FORWARD] {
            let (own, other) = (walk | key::LEFT, walk | key::RIGHT);
            // Left (east) ends in the band: refused; right (west) is clear.
            assert_eq!(
                choose(&tactics, from, own, key::LEFT, true),
                Step::Slid(other),
                "walk {walk:#x}"
            );
            assert_eq!(choose(&tactics, from, own, key::LEFT, false), Step::Stand);
            // The right side first is clear and taken as it is.
            assert_eq!(
                choose(&tactics, from, other, key::RIGHT, true),
                Step::Own(other)
            );
            // With the lip switch off the left side is clear: the old step.
            tactics.clear_step_sees_lip = false;
            assert_eq!(choose(&tactics, from, own, key::LEFT, true), Step::Own(own));
            tactics.clear_step_sees_lip = true;
        }
        // In the band, facing the lip (yaw toward +x): the sides run along it, both ends in the band.
        let facing = kin::yaw_of([1.0, 0.0]);
        let in_band = Vec3::new(55.7, 2.0, 33.0);
        let ask_along = |keys: u32, strafe: u32| {
            let to = in_band + side_step(keys, strafe, facing, true);
            tactics.clear_step_for(in_band, to, 0, Some(0), false)
        };
        assert!(!ask_along(key::LEFT, key::LEFT) && !ask_along(key::RIGHT, key::RIGHT));
        assert_eq!(
            choose_step(key::LEFT, key::LEFT, false, true, true, false, ask_along).0,
            Step::Stand
        );
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
            unseen_by: Some(from),
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

    /// A hiding spot is weighed by the share of the enemy's bullet that reaches it (A142), not only filtered: behind a
    /// crate a third of the bullet goes through (under the rule's 0.5) the spot is allowed, and a spot behind a wall
    /// that stops it is better. With `spot_cost` off the nearest allowed spot is the answer, as it was.
    #[test]
    fn a_hiding_spot_behind_what_stops_the_bullet_beats_one_behind_what_lets_a_third_through() {
        use crate::v026g::bots::tuning::{CoverRule, SpotCost};
        use mapdata::{Armor, FaceArmor, RayTree, World};
        let base = field();
        let (points, triangles) = mapdata::library::PrimitiveShape::Box { half: Vec3::new(0.3, 2.0, 3.0) }.tessellate();
        let boxes = [
            (Vec3::new(38.0, 2.0, 20.0), FaceArmor::Material(Armor::new(0.28).unwrap())),
            (Vec3::new(38.0, 2.0, 28.0), FaceArmor::Material(Armor::new(5.0).unwrap())),
        ];
        let tree = RayTree::from_armored_corners(boxes.iter().flat_map(|&(centre, armor)| {
            let points = &points;
            triangles.iter().map(move |t| (t.map(|i| points[i as usize] + centre), armor))
        }));
        let tactics = Tactics::new(Arc::new(World::assemble(base.map.clone(), base.grid.clone(), tree.into())), 1.0);
        let rule = |spot_cost| CoverRule {
            pierce: mapdata::Pierce::new(0.3125).unwrap(),
            max_share: 0.5,
            spot_cost,
        };
        let enemy = Vec3::new(50.0, 0.0, 22.0);
        let query = |cover| SpotQuery {
            out_of_fire_of: Some(enemy),
            cover: Some(cover),
            samples: 96,
            ..SpotQuery::ring(Vec3::new(30.0, 0.0, 22.0), 2.0, 12.0)
        };
        let share = |spot: &Spot| {
            tactics.line_of_fire_exact(Some(rule(None)), still_eye(enemy, false), still_eye(spot.at, false)).share
        };
        let (mut lower, mut partial_filter_picks) = (0, 0);
        for seed in 0..24 {
            let filter = tactics.spots(&query(rule(None)), 0, 1, &mut Rng::new(seed));
            let weighed = tactics.spots(&query(rule(Some(SpotCost::of(0.5)))), 0, 1, &mut Rng::new(seed));
            let (Some(filter), Some(weighed)) = (filter.first(), weighed.first()) else { continue };
            assert!(share(weighed) <= share(filter) + 1e-4, "seed {seed}: the weighed spot is never more exposed");
            if share(filter) > 0.05 {
                partial_filter_picks += 1;
                if share(weighed) < share(filter) - 0.05 {
                    lower += 1;
                }
            }
        }
        assert!(partial_filter_picks > 0, "the filter alone picks a spot a part of the bullet reaches");
        assert!(lower > 0, "and the weighed answer is a less exposed one");
    }

    fn end_node_y(tactics: &Tactics, node: NodeId) -> f32 {
        tactics.world.grid.position(node).y
    }

    /// A box zone on the flat field of [`field_with`] that always hits
    /// `per_s` of a body's health a second (a continuous hit on two parts).
    fn box_zone(name: &str, centre: Vec3, half: Vec3, per_s: f32) -> mapdata::Zone {
        use mapdata::{Rotation, Shape, ShapeKind, Zone, ZoneKind};
        Zone {
            kind: ZoneKind::Damage,
            name: name.into(),
            team: -1,
            position: centre,
            damage_type: "fire".into(),
            damage: Some(mapdata::Damage {
                max_hit: per_s / 2.0,
                parts: 2,
                continuous: true,
                ..mapdata::Damage::default()
            }),
            shapes: vec![Shape {
                kind: ShapeKind::Box,
                center: Vec3::new(centre.x, 1.0, centre.z),
                rotation: Rotation::IDENTITY,
                half,
            }],
        }
    }

    /// The hit a second of the zones on the floor a point of `route` is on.
    fn hits_along(tactics: &Tactics, route: &Route) -> Vec<f32> {
        let grid = &tactics.world.grid;
        let mut hits = Vec::new();
        let mut s = 0.0;
        while s <= route.length() {
            if let Some(node) = grid.locate(route.point_at(s), 1.0) {
                hits.push(tactics.hazards.hit_per_s(node));
            }
            s += 0.5;
        }
        hits
    }

    /// A step out of a grenade's blast: the nearest floor whose chest is 10.5 m from the
    /// rest point; the floor as far as it gets when none is in reach (and none when it already
    /// stands at the best of it); its own floor from outside.
    #[test]
    fn a_step_out_of_a_blast_goes_to_the_nearest_floor_outside_or_as_far_as_it_gets() {
        let world = field_with(|_| {});
        let tactics = Tactics::new(world.clone(), 1.0);
        let grid = &world.grid;
        let rest = Vec3::new(20.0, 0.0, 20.0);
        let open = RouteFilter::open();
        let from = grid.locate(Vec3::new(23.0, 0.0, 20.0), 1.0).unwrap();
        let out = tactics
            .step_out(from, rest, 10.5, 20.0, 0, Some(0), false, &open)
            .expect("a floor outside");
        let chest = Vec3::new(0.0, 1.3, 0.0);
        let reach = (out + chest - rest).length();
        assert!((10.5..12.0).contains(&reach), "the nearest outside, not far past: {out:?} {reach}");
        // A walk of 3 m only: no floor outside; the farthest in reach, about 3 m on from 3 m.
        let near = tactics
            .step_out(from, rest, 10.5, 3.0, 0, Some(0), false, &open)
            .expect("as far as it gets");
        let went = near.distance_xz(grid.position(from));
        assert!((2.0..3.2).contains(&went), "{near:?} {went}");
        assert!((near + chest - rest).length() > 3.0 + 1.5);
        // With no reach at all it stays where it is: none.
        assert_eq!(tactics.step_out(from, rest, 10.5, 0.0, 0, Some(0), false, &open), None);
        // Standing outside already, its own floor.
        let outside = grid.locate(Vec3::new(33.0, 0.0, 20.0), 1.0).unwrap();
        let stay = tactics
            .step_out(outside, rest, 10.5, 20.0, 0, Some(0), false, &open)
            .unwrap();
        assert_eq!(stay, grid.position(outside));
    }

    /// [`Tactics::step_out_routes`] is [`Tactics::step_out`] checked by the planned route: on the open field the same floor,
    /// the nearest by link distance; a reach too short for any floor outside gives the farthest in reach (the third pick); a
    /// reach too short for the nearest floor's route but not the link distance is not a floor it can count on.
    #[test]
    fn a_step_out_by_route_is_the_nearest_floor_whose_route_fits_the_reach() {
        let world = field_with(|_| {});
        let tactics = Tactics::new(world.clone(), 1.0);
        let grid = &world.grid;
        let rest = Vec3::new(20.0, 0.0, 20.0);
        let open = RouteFilter::open();
        let from = grid.locate(Vec3::new(23.0, 0.0, 20.0), 1.0).unwrap();
        let plain = tactics.step_out(from, rest, 10.5, 20.0, 0, Some(0), false, &open).unwrap();
        let (routed, pick) = tactics.step_out_routes(from, rest, 10.5, 20.0, 20.0, 0, Some(0), false, &open).unwrap();
        assert_eq!((routed, pick), (plain, StepOutPick::Nearest));
        // Too short for any floor outside: the farthest in reach, as step_out gives it.
        let near = tactics.step_out(from, rest, 10.5, 3.0, 0, Some(0), false, &open).unwrap();
        let (routed, pick) = tactics.step_out_routes(from, rest, 10.5, 3.0, 3.0, 0, Some(0), false, &open).unwrap();
        assert_eq!((routed, pick), (near, StepOutPick::Farthest));
        // A reach of 1 m by route but 3 m at the most: none outside, the farthest floor within the 3 m, as far as it gets.
        let (far, pick) = tactics.step_out_routes(from, rest, 10.5, 1.0, 3.0, 0, Some(0), false, &open).unwrap();
        assert_eq!((far, pick), (near, StepOutPick::Farthest));
        // Standing outside already: its own floor.
        let outside = grid.locate(Vec3::new(33.0, 0.0, 20.0), 1.0).unwrap();
        let (stay, pick) = tactics.step_out_routes(outside, rest, 10.5, 20.0, 20.0, 0, Some(0), false, &open).unwrap();
        assert_eq!((stay, pick), (grid.position(outside), StepOutPick::Nearest));
        // The planned route of the floor found is within the reach asked.
        let to = grid.locate(routed_floor(&tactics, from, rest), 1.0).unwrap();
        assert!(tactics.route_nodes(from, to, 0, Some(0), false, &open).unwrap().length() <= 3.0 + 1.0);
    }

    /// The floor [`Tactics::step_out_routes`] gives for a reach of 3 m from `from`.
    fn routed_floor(tactics: &Tactics, from: NodeId, rest: Vec3) -> Vec3 {
        tactics.step_out_routes(from, rest, 10.5, 3.0, 3.0, 0, Some(0), false, &RouteFilter::open()).unwrap().0
    }

    /// [A195] `Tactics::spot_exposure` counts the 30-degree sectors the enemy's route points 20-60 m off can see a spot from, not
    /// the anchor's own; behind the wall none, in the open some. A query with an `Exposure` ranks the spots seen from fewer
    /// bearings first and counts the lists; without one the order is as it was.
    #[test]
    fn a_spot_seen_from_fewer_bearings_ranks_first_with_an_exposure_term() {
        let world = field_with(|_| {});
        let mut tactics = Tactics::new(world, 1.0);
        // The enemy's (side 1) human route: east of the wall across x = 40 (z 0..25), along z = 10 and z = 35.
        let points: Vec<Vec3> = (0..=15)
            .flat_map(|k| {
                let x = 60.0 + k as f32;
                [Vec3::new(x, 0.0, 10.0), Vec3::new(x, 0.0, 35.0)]
            })
            .collect();
        tactics.set_routes(vec![super::super::routes::HumanRoute {
            team: 1,
            lives: 5,
            points: points.clone(),
            ..Default::default()
        }]);
        let exposure = Exposure::measured();
        let anchor = Vec3::new(30.0, 0.0, 30.0);
        let behind = tactics.spot_exposure(Vec3::new(30.0, 0.0, 10.0), anchor, &points, &exposure);
        let open = tactics.spot_exposure(Vec3::new(30.0, 0.0, 35.0), anchor, &points, &exposure);
        assert!(open > behind, "open {open} against behind the wall {behind}");
        // The ranking: the best given is seen from no more bearings than the best by nearness alone.
        let plain = SpotQuery {
            side: Some(0),
            ..SpotQuery::ring(Vec3::new(30.0, 0.0, 20.0), 6.0, 14.0)
        };
        let weighed = SpotQuery {
            exposure: Some(exposure),
            ..plain
        };
        let seen = |at: Vec3| tactics.spot_exposure(at, plain.around, &points, &exposure);
        let by_nearness = tactics.spots(&plain, 0, 6, &mut Rng::new(4));
        assert_eq!(tactics.cover_tally.exposure_lists.load(std::sync::atomic::Ordering::Relaxed), 0);
        let ranked = tactics.spots(&weighed, 0, 6, &mut Rng::new(4));
        assert_eq!(tactics.cover_tally.exposure_lists.load(std::sync::atomic::Ordering::Relaxed), 1);
        assert!(!by_nearness.is_empty() && ranked.len() == by_nearness.len());
        assert!(seen(ranked[0].at) <= seen(by_nearness[0].at));
        assert!(seen(ranked[0].at) < seen(by_nearness.iter().map(|s| s.at).max_by_key(|at| seen(*at)).unwrap()) || by_nearness.iter().all(|s| seen(s.at) == seen(ranked[0].at)));
        // Every candidate is weighed, not the first n to pass: the list is the best n of them, so it is no worse in bearings.
        let total = |spots: &[Spot]| spots.iter().map(|s| u32::from(seen(s.at))).sum::<u32>();
        assert!(total(&ranked) <= total(&by_nearness));
        // No route points of the enemy: the order is as it was.
        let mut bare = Tactics::new(field_with(|_| {}), 1.0);
        bare.set_routes(Vec::new());
        let same: Vec<_> = bare.spots(&weighed, 0, 6, &mut Rng::new(4)).iter().map(|s| s.node).collect();
        let plain_order: Vec<_> = bare.spots(&plain, 0, 6, &mut Rng::new(4)).iter().map(|s| s.node).collect();
        assert_eq!(same, plain_order);
    }

    /// From a mild zone the way out costs what it takes: not the nearest
    /// way, over a strong zone beside it, but the longer way out round it
    /// (the 0.90 stretch: a way out of a mild zone led down into a strong
    /// one).
    #[test]
    fn a_way_out_of_a_mild_zone_does_not_cross_a_strong_one() {
        let world = field_with(|map| {
            // The mild zone x 8-20, z 0-20, the strong one x 20-24 on its east
            // side: out east is the nearest (8.5 m) but 4 m of it in the strong
            // zone; out west is 9.5 m, north or south 11 m, in the mild one.
            map.zones.push(box_zone("mild", Vec3::new(14.0, 0.0, 10.0), Vec3::new(6.0, 3.0, 10.0), 0.02));
            map.zones.push(box_zone("strong", Vec3::new(22.0, 0.0, 10.0), Vec3::new(2.0, 3.0, 10.0), 0.8));
        });
        let tactics = Tactics::new(world.clone(), 1.0);
        let grid = &world.grid;
        let inside = grid.locate(Vec3::new(17.0, 0.0, 10.0), 1.0).unwrap();
        assert!(!tactics.allowed(inside, 0, Some(0), false));
        let out = tactics
            .escape(inside, None, 0, Some(0), false, &RouteFilter::open())
            .unwrap();
        let end = out.point_at(out.length());
        assert!(end.x < 20.0, "not east through the strong zone: {end:?}");
        let hits = hits_along(&tactics, &out);
        assert!(hits.iter().all(|&h| h <= 0.05 + 1e-4), "{hits:?}");
    }

    /// With the ceiling off (`EscapeRule::Nearest`) the way out is the old one:
    /// the nearest, east through the strong zone.
    #[test]
    fn with_the_ceiling_off_the_way_out_is_the_nearest() {
        let world = field_with(|map| {
            map.zones.push(box_zone("mild", Vec3::new(14.0, 0.0, 10.0), Vec3::new(6.0, 3.0, 10.0), 0.02));
            map.zones.push(box_zone("strong", Vec3::new(22.0, 0.0, 10.0), Vec3::new(2.0, 3.0, 10.0), 0.8));
        });
        let mut tactics = Tactics::new(world.clone(), 1.0);
        tactics.escape = crate::v026g::bots::tuning::EscapeRule::Nearest;
        let inside = world.grid.locate(Vec3::new(17.0, 0.0, 10.0), 1.0).unwrap();
        let out = tactics
            .escape(inside, None, 0, Some(0), false, &RouteFilter::open())
            .unwrap();
        let end = out.point_at(out.length());
        assert!(end.x > 24.0, "east, the nearest: {end:?}");
    }

    /// A body in a zone that every way out of crosses a stronger one of
    /// still gets out, the cheapest way: the ceiling is lifted.
    #[test]
    fn a_body_shut_in_by_a_stronger_zone_still_walks_out_the_cheapest_way() {
        let world = field_with(|map| {
            map.zones.push(box_zone("weak", Vec3::new(12.0, 0.0, 10.0), Vec3::new(2.0, 3.0, 2.0), 0.1));
            // A ring of strong zones about it, close enough that no floor
            // between is clear of both.
            for (x, z, hx, hz) in [
                (7.1, 10.0, 1.1, 5.0),
                (16.9, 10.0, 1.1, 5.0),
                (12.0, 12.9, 6.0, 1.1),
                (12.0, 7.1, 6.0, 1.1),
            ] {
                map.zones.push(box_zone("ring", Vec3::new(x, 0.0, z), Vec3::new(hx, 3.0, hz), 0.8));
            }
        });
        let tactics = Tactics::new(world.clone(), 1.0);
        let grid = &world.grid;
        let inside = grid.locate(Vec3::new(12.0, 0.0, 10.0), 1.0).unwrap();
        let out = tactics
            .escape(inside, None, 0, Some(0), false, &RouteFilter::open())
            .expect("a way out, over the ring");
        let end = out.point_at(out.length());
        let end_node = grid.locate(end, 0.5).unwrap();
        assert!(tactics.allowed(end_node, 0, Some(0), false));
        assert!(end.distance_xz(Vec3::new(12.0, 0.0, 10.0)) > 5.0, "{end:?}");
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
                .route_nodes(inside, to, 0, Some(0), false, &RouteFilter::open())
                .is_none()
        );
        let out = tactics.escape(inside, None, 0, Some(0), false, &RouteFilter::open()).unwrap();
        let end = out.point_at(out.length());
        let end_node = grid.locate(end, 0.3).unwrap();
        assert!(tactics.allowed(end_node, 0, Some(0), false));
        // The zone and its margin are 4 m round its centre: out within 5 m.
        assert!(out.length() < 5.0, "{}", out.length());
        // A landing the body gave up jumping to is kept off, by the search
        // for the way out and by the route to it.
        let shut = [Vec3::new(end.x, end_node_y(&tactics, end_node), end.z)];
        let other = tactics
            .escape(inside, None, 0, Some(0), false, &RouteFilter::shutting_landings(shut.iter().map(|&at| super::stuck::Shut::jump(at)).collect()))
            .unwrap();
        let end2 = other.point_at(other.length());
        assert!(end2.distance_xz(end) >= 0.35, "{end2:?} against {end:?}");
        assert!(
            other.points().iter().all(|p| p.at.distance_xz(end) >= 0.3),
            "{:?}",
            other.points()
        );
        // From there the goal has a way.
        assert!(
            tactics
                .route_nodes(end_node, to, 0, Some(0), false, &RouteFilter::open())
                .is_some()
        );
    }

    /// A route for a body that cannot sprint (a battery carrier) takes no jump
    /// link above the standing jump (`RouteFilter::with_max_jump_rise`): a
    /// 1.0 m platform is a jump away for the walker and no way for the
    /// carrier, a 0.9 m one is a way for both.
    #[test]
    fn a_carriers_route_takes_no_jump_above_the_standing_jump() {
        let route = |rise: f32, filter: &RouteFilter| {
            let solids = floor(10.0, 5.0, 14.0, 9.0, rise).to_vec();
            let world = field_with_solids(&solids, |_| {});
            let tactics = Tactics::new(world.clone(), 1.0);
            let grid = &world.grid;
            let from = grid.locate(Vec3::new(7.0, 0.0, 7.0), 0.5).unwrap();
            let to = grid.locate(Vec3::new(12.0, rise, 7.0), 0.3).unwrap();
            tactics.route_nodes(from, to, 0, Some(0), false, filter)
        };
        let carrier = RouteFilter::open().with_max_jump_rise(mapdata::grid::STANDING_JUMP_M);
        assert!(route(1.0, &RouteFilter::open()).is_some(), "the walker jumps onto 1.0 m");
        assert!(route(1.0, &carrier).is_none(), "the carrier has no way onto 1.0 m");
        assert!(route(0.9, &carrier).is_some(), "the carrier jumps onto 0.9 m");
    }

    /// A world with a platform 4 m up over x 20..32, z 28..38, a ramp to it
    /// from the ground at x 10..20, and a roof 4 m up no walk gets to at
    /// x 50..60, z 28..38.
    pub fn platform_world() -> Arc<World> {
        let walker = mapdata::WalkerBehaviour::BySlope;
        let quad = |a: Vec3, b: Vec3, c: Vec3, d: Vec3| {
            [
                mapdata::Triangle { v: [a, b, c], walker },
                mapdata::Triangle { v: [a, c, d], walker },
            ]
        };
        let mut triangles = floor(0.0, 0.0, 80.0, 40.0, 0.0).to_vec();
        triangles.extend(floor(20.0, 28.0, 32.0, 38.0, 4.0));
        triangles.extend(quad(
            Vec3::new(10.0, 0.0, 28.0),
            Vec3::new(20.0, 4.0, 28.0),
            Vec3::new(20.0, 4.0, 38.0),
            Vec3::new(10.0, 0.0, 38.0),
        ));
        triangles.extend(floor(50.0, 28.0, 60.0, 38.0, 4.0));
        // A step of 1.2 m (a jump up from the ground) and a second one
        // 1.2 m over it (2.4 m up): a ledge only a jump chain gets onto.
        triangles.extend(floor(42.0, 28.0, 47.0, 38.0, 1.2));
        triangles.extend(floor(47.0, 28.0, 51.0, 38.0, 2.4));
        let grid = WalkGrid::build(
            &triangles,
            [0.0, 0.0, 80.0, 40.0],
            0.25,
            mapdata::Body::default(),
        );
        let map = field().map.clone();
        Arc::new(World::assemble(
            map,
            grid.into(),
            RayTree::build(&triangles).into(),
        ))
    }

    /// V-S2a: a spot search looks at the storeys over the ring (up to
    /// `up_m`) only when asked and only with a sight line to keep; what it
    /// finds there is a floor a walk gets to, never the roof beside it.
    #[test]
    fn a_spot_search_finds_the_platform_over_the_ring_only_with_up_m() {
        let tactics = Tactics::new(platform_world(), 1.0);
        let anchor = Vec3::new(26.0, 0.0, 24.0);
        let query = |up_m: f32, sight_to: Option<Vec3>| SpotQuery {
            prefer_high: 1.0,
            up_m,
            sight_to,
            side: Some(0),
            samples: 600,
            ..SpotQuery::ring(anchor, 0.0, 14.0)
        };
        let ys = |q: &SpotQuery| -> Vec<f32> {
            tactics
                .spots(q, 0, 40, &mut Rng::new(3))
                .iter()
                .map(|s| s.at.y)
                .collect()
        };
        // The old search: the ground and the foot of the ramp (2 m).
        let before = ys(&query(0.0, None));
        assert!(!before.is_empty() && before.iter().all(|&y| y <= 2.01), "{before:?}");
        // Without a sight line to keep the storeys are not looked at.
        let blind = ys(&query(8.0, None));
        assert!(blind.iter().all(|&y| y <= 2.01), "{blind:?}");
        // With one (the ground and the platform's edge see the anchor).
        let seen = ys(&query(8.0, Some(anchor)));
        assert!(seen.iter().any(|&y| y > 3.5), "{seen:?}");
        // The platform ranks with the best of the ground (its credit is
        // capped), so the ground spots are still offered.
        assert!(seen.iter().any(|&y| y < 1.0), "{seen:?}");
        // And none of the spots up there is at the platform's edge.
        let grid = &tactics.world.grid;
        for spot in tactics.spots(&query(8.0, Some(anchor)), 0, 40, &mut Rng::new(3)) {
            if spot.at.y > 3.5 {
                assert!(!grid.near_fall(spot.node) && !grid.near_lip(spot.node), "{spot:?}");
            }
        }
        // The roof is no walk's, in or out of the window.
        let roof = SpotQuery {
            prefer_high: 1.0,
            up_m: 8.0,
            sight_to: Some(Vec3::new(55.0, 4.0, 33.0)),
            side: Some(0),
            ..SpotQuery::ring(Vec3::new(55.0, 0.0, 33.0), 0.0, 4.0)
        };
        let on_roof = tactics
            .spots(&roof, 0, 40, &mut Rng::new(3))
            .iter()
            .filter(|s| s.at.y > 3.0)
            .count();
        assert_eq!(on_roof, 0);
    }

    /// A ring round a point on the platform (4 m up) finds only the platform's own floor, and with `down_m` the ground
    /// under it too (`threat.job_spot_levels`: a camper up a storey left a job's ring with no floor).
    #[test]
    fn a_spot_search_finds_the_ground_under_the_ring_only_with_down_m() {
        let tactics = Tactics::new(platform_world(), 1.0);
        // By the platform's edge: the ground past it sees his point over the edge.
        let anchor = Vec3::new(31.0, 4.0, 33.0);
        let query = |down_m: f32| SpotQuery {
            down_m,
            sight_to: Some(anchor),
            side: Some(0),
            samples: 600,
            ..SpotQuery::ring(anchor, 4.0, 14.0)
        };
        let ys = |q: &SpotQuery| -> Vec<f32> { tactics.spots(q, 0, 60, &mut Rng::new(3)).iter().map(|s| s.at.y).collect() };
        let level = ys(&query(0.0));
        assert!(level.iter().all(|&y| y > 2.0), "{level:?}");
        let under = ys(&query(8.0));
        assert!(under.iter().any(|&y| y < 1.0), "{under:?}");
    }

    /// A storey only a jump chain gets onto is not offered over the ring:
    /// the bots' jumps are the weak part of their climbing (V-S2a smoke).
    #[test]
    fn a_storey_only_jumps_get_onto_is_not_offered_over_the_ring() {
        let tactics = Tactics::new(platform_world(), 1.0);
        let anchor = Vec3::new(47.0, 0.0, 24.0);
        let query = SpotQuery {
            prefer_high: 1.0,
            up_m: 8.0,
            sight_to: Some(anchor),
            side: Some(0),
            ..SpotQuery::ring(anchor, 0.0, 14.0)
        };
        let ys: Vec<f32> = tactics
            .spots(&query, 0, 60, &mut Rng::new(5))
            .iter()
            .map(|s| s.at.y)
            .collect();
        assert!(!ys.is_empty());
        assert!(ys.iter().all(|&y| y <= 2.0), "{ys:?}");
    }

    /// Decisions with no storey over the ring are the old search's: the
    /// same spots from the same stream.
    #[test]
    fn up_m_changes_nothing_where_no_storey_lies_over_the_ring() {
        let tactics = Tactics::new(field(), 1.0);
        let base = SpotQuery {
            prefer_high: 0.5,
            sight_to: Some(Vec3::new(30.0, 0.0, 30.0)),
            side: Some(0),
            ..SpotQuery::ring(Vec3::new(30.0, 0.0, 30.0), 5.0, 15.0)
        };
        let up = SpotQuery { up_m: 8.0, ..base };
        let a = tactics.spots(&base, 0, 20, &mut Rng::new(9));
        let b = tactics.spots(&up, 0, 20, &mut Rng::new(9));
        assert!(!a.is_empty());
        assert_eq!(a, b);
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

    /// The walk cache keeps the most recent goals only: the oldest goes out
    /// at the limit, and a goal asked again is not made twice.
    #[test]
    fn the_walk_cache_keeps_a_bounded_number_of_goals() {
        let mut cache = WalkCache::default();
        for goal in 0..5u32 {
            cache.keep(goal, Arc::new(vec![0.0; 10]), 3);
        }
        assert_eq!(cache.kept.len(), 3);
        assert!(cache.kept.contains_key(&4) && cache.kept.contains_key(&2));
        assert!(!cache.kept.contains_key(&0) && !cache.kept.contains_key(&1));
        cache.keep(4, Arc::new(vec![0.0; 10]), 3);
        assert_eq!(cache.order.len(), 3, "a goal kept again is not queued twice");
    }

    // The live zones (`live_zones.rs`): a world's anomaly fields next to the baked zones.

    fn live_field(id: u32, at: Vec3, radius: f32, phase: super::super::live_zones::FieldPhase) -> super::super::live_zones::WorldField {
        use super::super::live_zones::{FieldId, FieldKind, WorldField};
        use mapdata::{Rotation, Shape, ShapeKind};
        WorldField {
            id: FieldId(id),
            kind: FieldKind::Spring,
            shape: Shape {
                kind: ShapeKind::Cylinder,
                center: Vec3::new(at.x, 1.0, at.z),
                rotation: Rotation::IDENTITY,
                half: Vec3::new(radius, 2.0, radius),
            },
            phase,
        }
    }

    const LIVE_ON: super::super::live_zones::FieldPhase = super::super::live_zones::FieldPhase::Damaging { until: None };

    /// Floors only: a region is not shut because a live field lies on a few of its floors (its test is the baked
    /// zone's), while the floors' own test shuts them.
    #[test]
    fn a_live_field_shuts_floors_and_not_regions() {
        let world = field();
        let mut tactics = Tactics::new(world.clone(), 1.0);
        let grid = &world.grid;
        let inside = grid.locate(Vec3::new(20.0, 0.0, 20.0), 1.0).unwrap();
        let from = Vec3::new(5.0, 0.0, 20.0);
        let region = tactics.base.regions.of(inside);
        assert!(tactics.allowed(inside, 0, Some(0), false));
        let c = tactics.world.clone();
        tactics.hazards.set_fields(&[live_field(1, Vec3::new(20.0, 0.0, 20.0), 3.0, LIVE_ON)], &c.grid, 0);
        assert!(!tactics.allowed(inside, 0, Some(0), false), "the floor is shut");
        assert!(!tactics.allowed_from(inside, 0, Some(0), false, from), "and by the arrival test");
        assert!(tactics.region_allowed_from(region, 0, Some(0), false, from), "the region is open");
        assert!(tactics.hazards.zone_of(tactics.base.regions.middle(region)).is_none());
    }

    /// A body inside a live field gets out to the nearest floor that is open, and the route to it does not cross a
    /// second field.
    #[test]
    fn a_body_in_a_live_field_walks_out_and_not_through_a_second() {
        let world = field();
        let mut tactics = Tactics::new(world.clone(), 1.0);
        let grid = &world.grid;
        let c = tactics.world.clone();
        tactics.hazards.set_fields(
            &[
                live_field(1, Vec3::new(20.0, 0.0, 20.0), 3.0, LIVE_ON),
                live_field(2, Vec3::new(28.0, 0.0, 20.0), 3.0, LIVE_ON),
            ],
            &c.grid,
            0,
        );
        let inside = grid.locate(Vec3::new(20.0, 0.0, 20.0), 1.0).unwrap();
        let second = grid.locate(Vec3::new(28.0, 0.0, 20.0), 1.0).unwrap();
        assert!(!tactics.hazards.set_of(inside).is_none());
        assert_ne!(tactics.hazards.set_of(inside), tactics.hazards.set_of(second));
        let out = tactics
            .escape(inside, None, 0, Some(0), false, &RouteFilter::open())
            .expect("a way out");
        let end = out.point_at(out.length());
        let end_node = grid.locate(end, 0.5).unwrap();
        assert!(tactics.allowed(end_node, 0, Some(0), false), "out onto an open floor: {end:?}");
        let mut s = 0.0;
        while s <= out.length() {
            let node = grid.locate(out.point_at(s), 1.0).unwrap();
            assert!(tactics.hazards.live.at(node).all(|f| f.id.0 != 2), "not over the second field");
            s += 0.5;
        }
    }

    /// A body deeper in a live field than the breadth-first search reaches ([`ESCAPE_FLOORS`]) still gets out: the
    /// board has its own way out, since the exit field is the baked zones'.
    #[test]
    fn a_body_deep_in_a_wide_live_field_still_gets_out() {
        let world = field();
        let mut tactics = Tactics::new(world.clone(), 1.0);
        let grid = &world.grid;
        let c = tactics.world.clone();
        // 25 m round, on a floor 40 m across: the nearest open floor is 26 m off, past the 20,000 floors the escape
        // search expands before it gives up, so the way out is the board's own (`Tactics::live_exit_of`).
        tactics.hazards.set_fields(&[live_field(1, Vec3::new(30.0, 0.0, 20.0), 25.0, LIVE_ON)], &c.grid, 0);
        let inside = grid.locate(Vec3::new(30.0, 0.0, 20.0), 1.0).unwrap();
        assert!(tactics.exit_of(inside).is_some_and(|n| tactics.hazards.set_of(n).is_none()));
        let out = tactics
            .escape(inside, None, 0, Some(0), false, &RouteFilter::open())
            .expect("a way out");
        let end_node = grid.locate(out.point_at(out.length()), 0.5).unwrap();
        assert!(tactics.allowed(end_node, 0, Some(0), false));
    }

    /// A step out of a blast does not end on a live field's floor: the nearest floor outside the radius that is
    /// open, round the field.
    #[test]
    fn a_step_out_of_a_blast_avoids_a_live_field() {
        let world = field();
        let mut tactics = Tactics::new(world.clone(), 1.0);
        let grid = &world.grid;
        let rest = Vec3::new(20.0, 0.0, 20.0);
        let from = grid.locate(Vec3::new(23.0, 0.0, 20.0), 1.0).unwrap();
        let open = RouteFilter::open();
        let plain = tactics.step_out(from, rest, 10.5, 20.0, 0, Some(0), false, &open).unwrap();
        let c = tactics.world.clone();
        // A field right on where the step would end.
        tactics.hazards.set_fields(&[live_field(1, plain, 2.5, LIVE_ON)], &c.grid, 0);
        let plain_node = grid.locate(plain, 0.5).unwrap();
        assert!(!tactics.allowed(plain_node, 0, Some(0), false), "the plain step now ends on the field");
        let out = tactics.step_out(from, rest, 10.5, 20.0, 0, Some(0), false, &open).unwrap();
        let node = grid.locate(out, 0.5).unwrap();
        assert!(tactics.allowed(node, 0, Some(0), false), "not onto the field: {out:?}");
        assert!(out.distance_xz(plain) > 2.0, "{out:?} against {plain:?}");
    }

}
