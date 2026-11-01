//! Map knowledge for server-driven player bots (0.26g): spawns and
//! objectives, a walk grid with paths, and line of sight, from the map
//! projects in the game's `resources.db`. The notes are
//! `docs/notes/player_bots_mapdata.md` (how to dump, the file format, the
//! API, timings) and `docs/notes/player_bots.md` §5.
//!
//! - [`MapData`]: one project's objectives and placed collision. Loaded
//!   from `resources.db` ([`MapData::load`], the dump step) or from the
//!   file `mapdata-dump` writes ([`MapData::read`], the match server). The
//!   game's data is never part of this repository.
//! - [`WalkGrid`]: where the walking body stands and how it moves, from
//!   the walker mesh; [`WalkGrid::distances_to`] floods a target once for
//!   every bot heading there, [`WalkGrid::path`] is A* for one path.
//! - [`RayTree`]: line of sight on the bullet mesh ([`RayTree::visible`]).
//! - [`World`]: the three together for one match. Its grid and tree are
//!   `Arc`s: projects built from the same geometry share them
//!   ([`grid_key`], [`sight_key`]).
//!
//! Everything is in the map frame: left-handed, Y up, positions at the
//! feet. Values from the physics engine (the uploaded worlds' `m_last_pos`)
//! are in another frame, z negated, at the capsule's centre: see
//! [`math`] and [`PhysicsPoint`].
//!
//! ```no_run
//! use mapdata::{Body, MapData, Vec3, World};
//!
//! let map = MapData::read("maps/level_01_day_4.mapdata".as_ref()).unwrap();
//! let world = World::build(map, mapdata::CELL, Body::default());
//! let spawn = world.map.spawns[0].position;
//! let enemy = world.map.spawns.iter().find(|s| s.team == 1).unwrap().position;
//! let (from, to) = (world.grid.locate(spawn, 1.0).unwrap(), world.grid.locate(enemy, 1.0).unwrap());
//! let to_enemy = world.grid.distances_to(&[to]);
//! let (next, _how) = to_enemy.next_step(&world.grid, from).unwrap();
//! let eye = Vec3::new(0.0, 1.6, 0.0);
//! let sees = world.sight.visible(spawn + eye, enemy + eye);
//! # let _ = (next, sees);
//! ```

pub mod built;
mod config;
pub mod digest;
pub mod format;
pub mod grid;
pub mod library;
pub mod math;
pub mod pierce;
pub mod project;
pub mod regions;
pub mod resources;
pub mod sight;
pub mod stands;

pub use grid::{
    Body, BuildStats, DistanceMap, Erosion, ExitJump, LinkKind, NodeId, OffLink, OffMeshLinks, Path, PocketEntry, PressLead,
    PocketExit, PocketId, Search, SlicedSearch, WalkGrid,
};
pub use library::{Face, Library, Materials, Surface, WalkerBehaviour};
pub use pierce::{Armor, Crossing, DamageShare, FaceArmor, Load, Pierce};

/// Gives back the arrays this thread keeps between its searches (8 bytes a
/// floor of the largest grid searched, twice while a search is left under
/// way), for a thread whose searches are over for now; the next search
/// makes them again. With glibc, freed memory is also handed back to the
/// system (`malloc_trim`): it otherwise keeps blocks of a few MB for reuse.
pub fn free_search_memory() {
    grid::free_scratches();
    regions::free_marks();
    trim_freed_memory();
}

/// Hands memory the process has freed back to the system, with glibc
/// (`malloc_trim`): it otherwise keeps freed blocks of a few MB for reuse,
/// and the big ones a dropped world leaves. Nothing elsewhere.
pub fn trim_freed_memory() {
    #[cfg(all(target_os = "linux", target_env = "gnu"))]
    {
        unsafe extern "C" {
            fn malloc_trim(pad: usize) -> std::ffi::c_int;
        }
        // SAFETY: glibc's own call, safe at any time from any thread.
        unsafe {
            malloc_trim(0);
        }
    }
}

/// The bytes of the arrays this thread keeps between its searches
/// ([`free_search_memory`]).
pub fn search_memory_bytes() -> usize {
    grid::scratch_bytes() + regions::marks_bytes()
}
pub use math::{PhysicsPoint, Rotation, Transform, Vec3};
pub use project::{
    BulletMesh, Anomaly, AnomalyArtefacts, AnomalyEnergy, AnomalyState, ArtefactContainer, ArtefactCore,
    BatterySpawner, CollisionFilter, Container, Damage, MapData, Placement, Shape, ShapeKind,
    Spawn, Triangle, Zone, ZoneGroup, ZoneKind,
};
pub use resources::{Error, Resources};
pub use sight::{ArmorCensus, Followed, RayHit, RayTree};
pub use stands::{UsePose, UseStand, UseStands};

/// A play area margin around the `levelmap` box, metres.
pub const AREA_MARGIN: f32 = 5.0;

/// The walk grid's cell size to use, metres. 0.5 m cells shut 1 m doors
/// (level_02 and level_04 fall apart); 0.25 m cells keep them.
pub const CELL: f32 = 0.25;

/// The version of what [`WalkGrid::build`] gives, in [`grid_key`]: raised
/// with any change to the build that changes a grid (the pinned digest
/// test in `grid.rs` fails until then), so a stored or shared grid of the
/// old build is never taken for the new.
pub const GRID_ALGO: u32 = 11;
/// The same for the gap links' pass ([`grid::gap`]), in [`grid_key_with_links`]
/// only when a build makes them ([`OffMeshLinks::Gaps`]): a grid without
/// them keys, builds and writes as before they were made.
pub const GAP_LINKS_ALGO: u32 = 3;
/// The same for the walk-off lips ([`Body::walk_off_lips`]: a walk-off gap drop's take-off is a lip, so its floors and
/// those near it carry the near-lip flag the file stores), in [`grid_key_with_links`] only for a body that asks for it.
pub const WALK_OFF_LIPS_ALGO: u32 = 1;
/// The same for the Jump links kept only where the top they land on goes on as far as the jump carries the body
/// ([`Body::landing_carry`], `WalkGrid::drop_overshot_landings`), in [`grid_key_with_links`] only for a body that asks for it.
pub const LANDING_CARRY_ALGO: u32 = 3;
/// The same for the walks up a ledge's edge the capsule rides ([`Body::capsule_steps`]), in [`grid_key_with_links`] only for a
/// body that asks for it.
pub const CAPSULE_STEPS_ALGO: u32 = 1;
/// The same for the walk-offs kept only where the capsule has room to walk off the edge ([`Body::drop_room`],
/// `WalkGrid::cut_blocked_walk_offs`), in [`grid_key_with_links`] only for a body that asks for it.
pub const DROP_ROOM_ALGO: u32 = 1;
/// The same for the walks up off steep ground measured from where the climb began ([`Body::steep_walk_base`],
/// `WalkGrid::cut_steep_climbs`), in [`grid_key_with_links`] only for a body that asks for it.
pub const STEEP_WALK_BASE_ALGO: u32 = 1;
/// The same for the exact erosion pass ([`Erosion::Exact`], `grid::exact`), in [`grid_key_with_links`] only for a body
/// that asks for it: the grids of the cells' erosion key, build and write as before. 2: floors keep a quarter cell of
/// slack (the nearer-than-the-radius-less-a-quarter rule; 1 was the full radius). 3: the capsule's round top (a wall above the
/// cylinder is met at a smaller reach) and the floors only a crouched capsule fits by the walls kept, crouch-only.
pub const EXACT_EROSION_ALGO: u32 = 3;
/// The same for the honest drops pass (`Body::drop_flight_mps` over 0: a Drop is kept only if the body that walks off comes
/// down within the harmless fall, `WalkGrid::drop_deep_flights`), in [`grid_key_with_links`] only for a body that asks for it.
/// Raised with any change to which Drops that pass keeps; the pinned digest of `tests/pinned_passes.rs` fails until then.
pub const HONEST_DROPS_ALGO: u32 = 1;
/// The same for what only the clients' body builds (`Body::client_slope`: the sprint-jump links above the standing jump and
/// their run-ups, no jump landing on steep ground), in [`grid_key_with_links`] only for such a body. The body's numbers are in
/// the key already; the code that reads them is not, so a change to it needs this raised.
pub const CLIENT_BODY_ALGO: u32 = 1;
/// The same for [`RayTree::build`], in [`sight_key`]. 2: split by the surface area heuristic, boxes grown by a
/// millimetre, the split axis kept in a branch (the tree of 1 was median splits and tight boxes). 3: the tree keeps
/// each triangle's bullet armor ([`FaceArmor`]), in the key and in the file.
pub const SIGHT_ALGO: u32 = 3;

/// One map ready for bots: its data, walk grid and sight tree.
pub struct World {
    pub map: MapData,
    pub grid: std::sync::Arc<WalkGrid>,
    pub sight: std::sync::Arc<RayTree>,
}

impl World {
    /// Builds the grid (cells of `cell` metres over the play area, normally
    /// [`CELL`], keeping the floors a body gets to from the map's
    /// [`MapData::grid_seeds`], its pockets those of the spawns) and the
    /// sight tree: 0.3-0.8 s for a 0.26g map at 0.25 m (timings in the
    /// notes).
    pub fn build(map: MapData, cell: f32, body: Body) -> Self {
        Self::build_with_links(map, cell, body, OffMeshLinks::None)
    }

    /// [`Self::build`] with the off-mesh links `links` in its grid.
    pub fn build_with_links(map: MapData, cell: f32, body: Body, links: OffMeshLinks) -> Self {
        Self::build_with(map, cell, body, links, false)
    }

    /// [`Self::build_with_links`], with the grid's table of wall clearances ([`WalkGrid::wall_clearance`]) when `wall_clearance`
    /// is set (0.5-1.7 s a map).
    pub fn build_with(map: MapData, cell: f32, body: Body, links: OffMeshLinks, wall_clearance: bool) -> Self {
        let spawns: Vec<Vec3> = map.spawns.iter().map(|s| s.position).collect();
        let walker = map.walker_triangles();
        let (grid, _) = WalkGrid::build_kept_with_links(
            &walker,
            map.play_area(AREA_MARGIN),
            cell,
            body,
            &map.grid_seeds(),
            &spawns,
            links,
        );
        let grid = if wall_clearance { grid.with_wall_clearance(&walker) } else { grid };
        let sight = map.bullet_mesh().build_tree();
        Self::assemble(map, grid.into(), sight.into())
    }

    /// A world of a grid and a tree built before, from this map's geometry
    /// (or the same geometry: [`grid_key`], [`sight_key`]).
    pub fn assemble(
        map: MapData,
        grid: std::sync::Arc<WalkGrid>,
        sight: std::sync::Arc<RayTree>,
    ) -> Self {
        Self { map, grid, sight }
    }
}

/// What a walk grid is built from, as one number: the triangles as placed
/// (corners by their bits, walker behaviour), the area, the cell, the body,
/// the seeds ([`WalkGrid::build_kept`]) and [`GRID_ALGO`]. Equal keys build
/// equal grids (the build is deterministic:
/// `every_world_builds_the_same_twice`).
pub fn grid_key(
    triangles: &[Triangle],
    area: [f32; 4],
    cell: f32,
    body: Body,
    seeds: &[Vec3],
) -> u64 {
    grid_key_with_links(triangles, area, cell, body, seeds, OffMeshLinks::None)
}

/// [`grid_key`] of a grid built with the off-mesh links `links`
/// ([`WalkGrid::build_kept_with_links`]); with none, the same number.
pub fn grid_key_with_links(
    triangles: &[Triangle],
    area: [f32; 4],
    cell: f32,
    body: Body,
    seeds: &[Vec3],
    links: OffMeshLinks,
) -> u64 {
    let mut digest = digest::Digest::new();
    digest.u32(GRID_ALGO);
    match links {
        OffMeshLinks::None => {}
        OffMeshLinks::Gaps => digest.u32(GAP_LINKS_ALGO),
    }
    triangles_into(&mut digest, triangles, true);
    for value in area.into_iter().chain([cell]) {
        digest.f32(value);
    }
    for value in body.values() {
        digest.f32(value);
    }
    if body.drop_flight_mps > 0.0 {
        digest.f32(body.drop_flight_mps);
        digest.u32(HONEST_DROPS_ALGO);
    }
    // The cells' erosion is the default: its keys are the ones there were.
    if body.erosion == Erosion::Exact {
        digest.u32(EXACT_EROSION_ALGO);
        // The floors only a crouched capsule fits are taken away: another grid (a key part only then).
        if !body.crouch_only_floors {
            digest.u32(EXACT_EROSION_ALGO + 100);
        }
    }
    // The lips at the walk-offs are a switch: the keys of grids without them are the ones there were.
    if body.walk_off_lips {
        digest.u32(WALK_OFF_LIPS_ALGO);
    }
    // The capsule's edge steps are a switch too.
    if body.capsule_steps {
        digest.u32(CAPSULE_STEPS_ALGO);
    }
    // The room to walk off is a switch too.
    if body.drop_room {
        digest.u32(DROP_ROOM_ALGO);
    }
    // The walks up off steep ground from where the climb began too.
    if body.steep_walk_base {
        digest.u32(STEEP_WALK_BASE_ALGO);
    }
    // The carried landings are a switch too.
    if body.landing_carry {
        digest.u32(LANDING_CARRY_ALGO);
        // Without the last-resort links is another grid (a key part only then: the keys of the others are the ones there were).
        if !body.keep_last_resorts {
            digest.u32(LANDING_CARRY_ALGO + 100);
        }
    }
    // The default body's keys are the ones there were.
    if body.client_slope() {
        digest.u32(CLIENT_BODY_ALGO);
    }
    digest.len(seeds.len());
    for seed in seeds {
        digest.f32(seed.x);
        digest.f32(seed.y);
        digest.f32(seed.z);
    }
    digest.finish()
}

/// What a sight tree is built from ([`grid_key`] for the tree): the
/// triangles' corners, their armor and [`SIGHT_ALGO`]. `armor` is one per triangle ([`BulletMesh`] keeps them
/// together).
pub fn sight_key(triangles: &[Triangle], armor: &[FaceArmor]) -> u64 {
    debug_assert_eq!(triangles.len(), armor.len());
    let mut digest = digest::Digest::new();
    digest.u32(SIGHT_ALGO);
    triangles_into(&mut digest, triangles, false);
    for armor in armor {
        digest.f32(armor.to_wire());
    }
    digest.finish()
}

fn triangles_into(digest: &mut digest::Digest, triangles: &[Triangle], walker: bool) {
    digest.len(triangles.len());
    for triangle in triangles {
        for v in triangle.v {
            digest.f32(v.x);
            digest.f32(v.y);
            digest.f32(v.z);
        }
        if walker {
            digest.u32(triangle.walker as u32);
        }
    }
}
