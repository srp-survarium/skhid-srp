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
pub mod controller;
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
    Body, BuildStats, DistanceMap, Erosion, ExitJump, ExitReach, LinkKind, NodeId, OffLink, OffMeshLinks, Path, PocketEntry, PressLead,
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
/// The same for the floors reached by a slip off a walk-off's take-off ([`Body::slip_landings`], `WalkGrid::slip_landings`), in
/// [`grid_key_with_links`] only for a body that asks for it. A value of its own: a key with only this flag is not one with only
/// another flag whose value is 1. 2: a slip comes down only on a floor open above the take-off (A247).
pub const SLIP_LANDINGS_ALGO: u32 = 0x5119_0002;
/// The same for the sprint jumps' run-ups asked after the erosion ([`Body::run_ups_after_erosion`], `WalkGrid::drop_short_run_ups`),
/// in [`grid_key_with_links`] only for a body that asks for it.
pub const RUN_UPS_AFTER_EROSION_ALGO: u32 = 1;
/// The same for the walks up off steep ground measured from where the climb began ([`Body::steep_walk_base`],
/// `WalkGrid::cut_steep_climbs`), in [`grid_key_with_links`] only for a body that asks for it.
pub const STEEP_WALK_BASE_ALGO: u32 = 1;
/// The same for the pockets shut by the exits a body can take ([`Body::pocket_exits_walled`], `WalkGrid::mark_no_exit`), in
/// [`grid_key_with_links`] only for a body that asks for it. A value of its own.
pub const POCKET_EXITS_WALLED_ALGO: u32 = 0x5270_0001;
/// The same for the pockets' exits capped at the sprint jump's lowest top and kept only where a body can take them
/// ([`Body::exits_in_reach`], A347), in [`grid_key_with_links`] only for a body that asks for it. A value of its own (2: the
/// walled exits).
pub const EXITS_IN_REACH_ALGO: u32 = 0x5347_0002;
/// The same for the gap links down a flight made only over a face that held the body ([`Body::gap_no_vaults`], A293), in
/// [`grid_key_with_links`] only for a body that asks for it. A value of its own.
pub const GAP_NO_VAULTS_ALGO: u32 = 0x5293_0001;
/// The same for the ramp slopes kept by floor ([`Body::ramp_faces`], A304), in [`grid_key_with_links`] only for a body that asks
/// for it. A value of its own.
pub const RAMP_FACES_ALGO: u32 = 0x5304_0101;
/// The same for the links the client's walk tick decides ([`Body::client_controller`], A313), in [`grid_key_with_links`] only for
/// a body that asks for it. A value of its own.
pub const CLIENT_CONTROLLER_ALGO: u32 = 0x5313_0001;
/// The same for the walk-off walls with the steep `walk` faces ([`Body::drop_room_walk_faces`], A304), in [`grid_key_with_links`]
/// only for a body that asks for it. A value of its own.
pub const DROP_ROOM_WALK_FACES_ALGO: u32 = 0x5304_0201;
/// The same for the narrow doors' floors given back after the erosion rounds ([`Body::narrow_doors`], A313), in
/// [`grid_key_with_links`] only for a body that asks for it. A value of its own.
pub const NARROW_DOORS_ALGO: u32 = 0x5313_0101;
/// The same for the Drop links the client's walk tick does not take a body down ([`Body::drop_check`], A313), in
/// [`grid_key_with_links`] only for a body that asks for it. A value of its own.
pub const DROP_CHECK_ALGO: u32 = 0x5313_0201;
/// The same for the floors given back beside walls the body clears ([`Body::wall_give_back`], A318), in [`grid_key_with_links`]
/// only for a body that asks for it. A value of its own.
pub const WALL_GIVE_BACK_ALGO: u32 = 0x5318_0102;
/// The same for the walk-tick passes with the exe's collision rules ([`Body::exe_collision`], A325), in
/// [`grid_key_with_links`] only for a body that asks for it. A value of its own.
pub const EXE_COLLISION_ALGO: u32 = 0x5325_0101;
/// The same for the client-controller pass's start clear of every face ([`Body::walk_start_clear`], A329), in
/// [`grid_key_with_links`] only for a body that asks for it. A value of its own.
pub const WALK_START_ALGO: u32 = 0x5329_0101;
/// The same for the bridges across cracks the grid crossed by a drop ([`Body::crack_bridges`], A329), in
/// [`grid_key_with_links`] only for a body that asks for it. A value of its own.
pub const CRACK_BRIDGE_ALGO: u32 = 0x5329_0201;
/// The same for the Bullet caster's start-overlap witness on the face ([`Body::start_witness`], A329), in
/// [`grid_key_with_links`] only for a body that asks for it with [`Body::exe_collision`]. A value of its own.
pub const START_WITNESS_ALGO: u32 = 0x5329_0301;
/// The same for the rounds-taken floors given back where a body stands clear inside the cell ([`Body::give_back_in_cell`],
/// A331), in [`grid_key_with_links`] only for a body that asks for it with [`Body::wall_give_back`]. A value of its own.
pub const GIVE_BACK_IN_CELL_ALGO: u32 = 0x5331_0101;
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

/// [A346] What a grid gets once its links are built or read back, before a world uses it: tables derived from the walker
/// triangles, in neither the built file nor the grid key. The one step that the server's map store (`bots::maps`) and every
/// world built here (the bench's matches, the tests) finish their grids with, so a setting the store applies cannot be missing
/// from the others (twice it was: the client walker, A318, and the pull hold, A333).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct GridExtras {
    /// The table of wall clearances ([`WalkGrid::wall_clearance`]; 0.5-1.7 s a map).
    pub wall_clearance: bool,
    /// The table of open sides ([`WalkGrid::open_toward`]) for a capsule held this far past a floor's centre at a ledge.
    pub open_sides: Option<f32>,
    /// [A333] The pull's hold for the routes' straight legs ([`WalkGrid::with_pull_hold`]; it needs the wall clearances).
    pub pull_hold: Option<f32>,
    /// [A334] The routes' pulled legs walked by the client's walker ([`WalkGrid::with_pull_walk`]; the walker is always kept).
    pub pull_walk: bool,
}

impl GridExtras {
    /// `grid` with these tables, and the client's walker always ([`WalkGrid::with_client_walker`]: the reckoning's client step,
    /// A318), all from the map's `walker` triangles.
    pub fn apply(self, grid: WalkGrid, walker: &[Triangle]) -> WalkGrid {
        let grid = if self.wall_clearance { grid.with_wall_clearance(walker) } else { grid };
        let grid = match self.pull_hold {
            Some(hold) => grid.with_pull_hold(hold),
            None => grid,
        };
        let grid = match self.open_sides {
            Some(support_m) => grid.with_open_sides(walker, support_m),
            None => grid,
        };
        let grid = grid.with_client_walker(walker);
        if self.pull_walk { grid.with_pull_walk() } else { grid }
    }

    /// Which of these tables `grid` holds (wall clearances, open sides, the walker's bytes, the pull's hold): equal for two
    /// grids finished alike.
    pub fn held_by(grid: &WalkGrid) -> (bool, bool, usize, Option<f32>, bool) {
        (grid.has_wall_clearance(), grid.has_open_sides(), grid.client_walker_bytes(), grid.pull_hold(), grid.pull_walk())
    }
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
        Self::build_with(map, cell, body, links, false, None)
    }

    /// [`Self::build_with_links`], with the grid's table of wall clearances ([`WalkGrid::wall_clearance`]) when `wall_clearance`
    /// is set (0.5-1.7 s a map), and its table of open sides ([`WalkGrid::open_toward`]) for a capsule held `support_m` past a
    /// floor's centre at a ledge when `open_sides` is `Some(support_m)`.
    pub fn build_with(map: MapData, cell: f32, body: Body, links: OffMeshLinks, wall_clearance: bool, open_sides: Option<f32>) -> Self {
        Self::build_with_extras(map, cell, body, links, GridExtras { wall_clearance, open_sides, pull_hold: None, pull_walk: false })
    }

    /// [`Self::build_with_links`], the grid finished by `extras` ([`GridExtras::apply`]) as the server's map store finishes its
    /// own (`bots::maps`): the same step, so a table the store adds cannot be missing from a world built here.
    pub fn build_with_extras(map: MapData, cell: f32, body: Body, links: OffMeshLinks, extras: GridExtras) -> Self {
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
        let grid = extras.apply(grid, &walker);
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
    // The floors a slip reaches are a switch too.
    if body.slip_landings {
        digest.u32(SLIP_LANDINGS_ALGO);
    }
    // So is the run-ups' place in the build.
    if body.run_ups_after_erosion {
        digest.u32(RUN_UPS_AFTER_EROSION_ALGO);
    }
    // The walks up off steep ground from where the climb began too.
    if body.steep_walk_base {
        digest.u32(STEEP_WALK_BASE_ALGO);
    }
    // And the pockets shut by the exits a body can take.
    if body.pocket_exits_walled {
        digest.u32(POCKET_EXITS_WALLED_ALGO);
    }
    // And the exits capped at the sprint jump's lowest top.
    if body.exits_in_reach {
        digest.u32(EXITS_IN_REACH_ALGO);
    }
    // And the gap links down over a face that held the body.
    if body.gap_no_vaults {
        digest.u32(GAP_NO_VAULTS_ALGO);
    }
    // And the ramp slopes.
    if body.ramp_faces {
        digest.u32(RAMP_FACES_ALGO);
    }
    // And the links of the client's walk tick.
    if body.client_controller {
        digest.u32(CLIENT_CONTROLLER_ALGO);
    }
    // And the walk-off walls with the `walk` faces.
    if body.drop_room_walk_faces {
        digest.u32(DROP_ROOM_WALK_FACES_ALGO);
    }
    // And the narrow doors' floors.
    if body.narrow_doors {
        digest.u32(NARROW_DOORS_ALGO);
    }
    // And the walk tick's check of the Drop links.
    if body.drop_check {
        digest.u32(DROP_CHECK_ALGO);
    }
    // And the floors given back beside walls.
    if body.wall_give_back {
        digest.u32(WALL_GIVE_BACK_ALGO);
        // And where in the cell a body may stand.
        if body.give_back_in_cell {
            digest.u32(GIVE_BACK_IN_CELL_ALGO);
        }
    }
    // And the walk tick's collision rules.
    if body.exe_collision {
        digest.u32(EXE_COLLISION_ALGO);
        // And where its caster puts a contact met overlapping.
        if body.start_witness {
            digest.u32(START_WITNESS_ALGO);
        }
    }
    // And where the client-controller pass starts a body.
    if body.walk_start_clear {
        digest.u32(WALK_START_ALGO);
    }
    // And the bridges across cracks.
    if body.crack_bridges {
        digest.u32(CRACK_BRIDGE_ALGO);
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
