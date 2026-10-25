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
pub mod project;
pub mod regions;
pub mod resources;
pub mod sight;

pub use grid::{
    Body, BuildStats, DistanceMap, ExitJump, LinkKind, NodeId, Path, PocketEntry, PocketExit,
    PocketId, Search, SlicedSearch, WalkGrid,
};
pub use library::{Library, Materials, WalkerBehaviour};

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
    Anomaly, AnomalyArtefacts, AnomalyEnergy, AnomalyState, BatterySpawner, CollisionFilter,
    Container, Damage, MapData, Placement, Shape, ShapeKind, Spawn, Triangle, Zone, ZoneGroup,
    ZoneKind,
};
pub use resources::{Error, Resources};
pub use sight::RayTree;

/// A play area margin around the `levelmap` box, metres.
pub const AREA_MARGIN: f32 = 5.0;

/// The walk grid's cell size to use, metres. 0.5 m cells shut 1 m doors
/// (level_02 and level_04 fall apart); 0.25 m cells keep them.
pub const CELL: f32 = 0.25;

/// The version of what [`WalkGrid::build`] gives, in [`grid_key`]: raised
/// with any change to the build that changes a grid (the pinned digest
/// test in `grid.rs` fails until then), so a stored or shared grid of the
/// old build is never taken for the new.
pub const GRID_ALGO: u32 = 12;
/// The same for [`RayTree::build`], in [`sight_key`].
pub const SIGHT_ALGO: u32 = 1;

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
        let spawns: Vec<Vec3> = map.spawns.iter().map(|s| s.position).collect();
        let grid = WalkGrid::build_kept(
            &map.walker_triangles(),
            map.play_area(AREA_MARGIN),
            cell,
            body,
            &map.grid_seeds(),
            &spawns,
        );
        let sight = RayTree::build(&map.bullet_triangles());
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
    let mut digest = digest::Digest::new();
    digest.u32(GRID_ALGO);
    triangles_into(&mut digest, triangles, true);
    for value in area.into_iter().chain([cell]) {
        digest.f32(value);
    }
    for value in body.values() {
        digest.f32(value);
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
/// triangles' corners and [`SIGHT_ALGO`].
pub fn sight_key(triangles: &[Triangle]) -> u64 {
    let mut digest = digest::Digest::new();
    digest.u32(SIGHT_ALGO);
    triangles_into(&mut digest, triangles, false);
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
