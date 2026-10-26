//! The maps the server's bots path on (`SRP_MAPDATA_DIR`,
//! `docs/notes/player_bots_mapdata.md` §6): one [`mapdata::World`] and its
//! [`TacticsBase`] per project, built from `<dir>/<project>.mapdata` (the
//! file `mapdata-dump` writes) on a thread of its own. A build takes
//! 0.3-0.8 s and the match loop is one thread, so a caller never waits: it
//! asks early ([`Maps::request`]) and looks again each tick
//! ([`Maps::get`]) until the map is [`MapState::Ready`].
//!
//! Every match on a map shares its world and its tactics (for the default
//! hazard margin; a match tuned with another builds its own tactics on the
//! shared world). Projects built from the same geometry (the weather
//! variants of most levels: 72 projects, 29 walk grids, 30 sight trees)
//! share one grid and one tree: they are found by the key of what they are
//! built from ([`mapdata::grid_key`], [`mapdata::sight_key`]), which builds
//! the same every time. The maps' meshes are dropped once the grid and the
//! tree are built: nothing reads them later.
//!
//! A grid or tree built is written to `<dir>/built/` (`SRP_MAPDATA_BUILT`)
//! by its key
//! (`mapdata::built`), and read from there the next time instead of built:
//! a tenth of the time, and no build peak. A file is checked index by index
//! and by its digest, so it answers as the build would; one that fails is
//! reported, built again and written over. The files are made by the
//! server itself, on the host, from the same `.mapdata` files.
//!
//! A world holds 63-149 MB (a build peaks near twice that) and the 0.26g
//! host has under 1 GB, so the store keeps only the last [`KEEP`] used
//! (`SRP_MAPDATA_KEEP`) and builds one map at a time. A match holds its
//! map by its `Arc`; one dropped from the store while a match still has
//! it is taken back without a new build. A file that is missing or fails
//! to load is reported once and stays absent until the server restarts.
//!
//! At most [`AT_ONCE`] maps are in memory (`SRP_MAPDATA_AT_ONCE`; the host
//! has room for one: `player_bots_mapdata.md`, "On host-like limits"). A
//! match on a map in memory has it at once. One that needs another map
//! while the maps in memory are all held by matches (or loading) waits
//! ([`MapState::Waiting`]): its bots run the stage 0 script, as while a
//! map loads, and take the map when it is loaded. A map no match holds is
//! dropped to make room, and the memory it freed handed back.

use std::collections::HashMap;
use std::fmt;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard, Weak};
use std::time::Instant;

use mapdata::{MapData, RayTree, WalkGrid, World};

use super::tactics::TacticsBase;
use super::tuning::Tuning;

/// How many built maps the store keeps by default, beyond those running
/// matches hold. With 72 projects the next match is rarely on the last
/// map, and a rebuild costs 0.3-0.8 s off the match thread.
pub const KEEP: usize = 1;

/// How many maps may be in memory at once by default, held by matches,
/// loading or kept.
pub const AT_ONCE: usize = 1;

/// The folder under `SRP_MAPDATA_DIR` of the built grids and trees.
pub const BUILT_DIR: &str = "built";

/// Where one project's map stands.
#[derive(Clone)]
pub enum MapState {
    /// No world for this project: no directory set, no file, or the load
    /// failed. The reason is for the log.
    Absent(String),
    /// Being built; look again later.
    Loading,
    /// Not loaded while the maps in memory are held by other matches
    /// ([`AT_ONCE`]); look again later. The reason is for the log.
    Waiting(String),
    /// Its world and its tactics for the default hazard margin.
    Ready(Arc<TacticsBase>),
}

impl fmt::Debug for MapState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Absent(reason) => f.debug_tuple("Absent").field(reason).finish(),
            Self::Loading => f.write_str("Loading"),
            Self::Waiting(reason) => f.debug_tuple("Waiting").field(reason).finish(),
            Self::Ready(base) => write!(f, "Ready({})", base.world.map.project),
        }
    }
}

/// The store of built maps, shared by every match of the server.
pub struct Maps {
    dir: Option<PathBuf>,
    /// Where built grids, trees and tactics are kept; `None`: built at
    /// each load.
    built: Option<PathBuf>,
    keep: usize,
    /// At most this many maps in memory ([`AT_ONCE`]).
    at_once: usize,
    cache: Arc<Mutex<Cache>>,
    /// Held by the build under way: one at a time, for memory and the one
    /// core of the host.
    builds: Arc<Mutex<()>>,
}

#[derive(Default)]
struct Cache {
    entries: HashMap<String, Entry>,
    /// Counts uses, for the least recently used.
    clock: u64,
    /// Every grid and tree built, by the key of what it is built from,
    /// while something holds it.
    grids: HashMap<u64, Weak<WalkGrid>>,
    sights: HashMap<u64, Weak<RayTree>>,
}

enum Entry {
    Loading,
    Ready {
        base: Arc<TacticsBase>,
        used: u64,
    },
    /// Dropped from the store; a match may still hold it (its tactics, or
    /// only its world when it built tactics of its own).
    Dropped(Weak<TacticsBase>, Weak<World>),
    Failed(String),
}

impl Maps {
    /// A store loading from `dir`; `None`: every project is absent.
    pub fn new(dir: Option<PathBuf>) -> Arc<Self> {
        let built = dir.as_ref().map(|dir| dir.join(BUILT_DIR));
        Self::configured(dir, KEEP, AT_ONCE, built)
    }

    /// As [`Maps::new`], keeping `keep` maps (at least one), with no bound
    /// on the maps in memory at once.
    pub fn with_keep(dir: Option<PathBuf>, keep: usize) -> Arc<Self> {
        let built = dir.as_ref().map(|dir| dir.join(BUILT_DIR));
        Self::configured(dir, keep, usize::MAX, built)
    }

    /// As [`Maps::with_keep`], at most `at_once` maps in memory (at least
    /// one), the built files in `built` (none: built at each load).
    pub fn configured(
        dir: Option<PathBuf>,
        keep: usize,
        at_once: usize,
        built: Option<PathBuf>,
    ) -> Arc<Self> {
        Arc::new(Self {
            dir,
            built,
            keep: keep.max(1),
            at_once: at_once.max(1),
            cache: Arc::default(),
            builds: Arc::default(),
        })
    }

    pub fn dir(&self) -> Option<&Path> {
        self.dir.as_deref()
    }

    /// The hazard margin the shared tactics are built for: the tuning's
    /// default. A match tuned otherwise builds its own.
    pub fn margin() -> f32 {
        Tuning::default().hazard.margin_m.get()
    }

    /// Starts building `project`'s map (`level_01/day_4`) unless it is
    /// built, being built or known to be absent.
    pub fn request(&self, project: &str) {
        self.get(project);
    }

    /// Where `project`'s map stands; starts building it as
    /// [`Maps::request`] does. Never blocks on a build.
    pub fn get(&self, project: &str) -> MapState {
        let Some(dir) = &self.dir else {
            return MapState::Absent("SRP_MAPDATA_DIR is not set".into());
        };
        let mut cache = lock(&self.cache);
        cache.clock += 1;
        let clock = cache.clock;
        let revived = match cache.entries.get_mut(project) {
            Some(Entry::Loading) => return MapState::Loading,
            Some(Entry::Failed(reason)) => return MapState::Absent(reason.clone()),
            Some(Entry::Ready { base, used }) => {
                *used = clock;
                return MapState::Ready(base.clone());
            }
            Some(Entry::Dropped(base, _)) => base.upgrade(),
            None => None,
        };
        if let Some(base) = revived {
            cache.insert_ready(project, base.clone(), self.keep);
            return MapState::Ready(base);
        }
        // A new map in memory: only with room for it.
        let held = cache.held_except(project);
        if held.len() >= self.at_once {
            return MapState::Waiting(format!(
                "{} in use; at most {} bots' map{} at once",
                held.join(", "),
                self.at_once,
                if self.at_once == 1 { "" } else { "s" }
            ));
        }
        if cache.drop_idle(project, self.at_once - held.len() - 1) {
            // What the dropped maps freed back to the system before the
            // next is read.
            mapdata::trim_freed_memory();
        }
        let path = dir.join(mapdata::format::file_name(project));
        if !path.is_file() {
            let reason = format!("no map file {}", path.display());
            log::warn!("Bot map {project}: {reason}; bots on it keep their script");
            cache
                .entries
                .insert(project.to_owned(), Entry::Failed(reason.clone()));
            return MapState::Absent(reason);
        }
        cache.entries.insert(project.to_owned(), Entry::Loading);
        drop(cache);
        self.spawn_build(project, path);
        MapState::Loading
    }

    fn spawn_build(&self, project: &str, path: PathBuf) {
        let (cache, builds) = (self.cache.clone(), self.builds.clone());
        let (keep, name) = (self.keep, project.to_owned());
        let built = self.built.clone();
        let spawned = std::thread::Builder::new()
            .name(format!("map {project}"))
            .spawn(move || {
                let result = {
                    let _turn = builds.lock().unwrap_or_else(|p| p.into_inner());
                    build(&path, built.as_deref(), &cache)
                };
                let mut cache = lock(&cache);
                match result {
                    Ok((base, built)) => {
                        log::info!(
                            "Bot map {name} ready in {:.2} s ({}), {} MB with its tactics",
                            built.seconds,
                            built.how(),
                            (world_bytes(&base.world) + base.memory_bytes()) >> 20
                        );
                        cache.insert_ready(&name, Arc::new(base), keep);
                    }
                    Err(reason) => {
                        log::warn!("Bot map {name}: {reason}; bots on it keep their script");
                        cache.entries.insert(name, Entry::Failed(reason));
                    }
                }
            });
        if let Err(error) = spawned {
            let reason = format!("can't start its build: {error}");
            log::warn!("Bot map {project}: {reason}");
            lock(&self.cache)
                .entries
                .insert(project.to_owned(), Entry::Failed(reason));
        }
    }
}

impl Cache {
    /// Puts `base` in as the latest used, and drops the least recently
    /// used ready maps over `keep`.
    fn insert_ready(&mut self, project: &str, base: Arc<TacticsBase>, keep: usize) {
        self.clock += 1;
        let used = self.clock;
        self.entries
            .insert(project.to_owned(), Entry::Ready { base, used });
        loop {
            let ready = self.entries.iter().filter_map(|(name, entry)| match entry {
                Entry::Ready { used, .. } => Some((*used, name)),
                _ => None,
            });
            if ready.clone().count() <= keep {
                break;
            }
            let Some((_, oldest)) = ready.min() else {
                break;
            };
            let oldest = oldest.clone();
            if let Some(Entry::Ready { base, .. }) = self.entries.remove(&oldest) {
                log::info!("Bot map {oldest} dropped from the store (kept {keep})");
                let world = Arc::downgrade(&base.world);
                self.entries
                    .insert(oldest, Entry::Dropped(Arc::downgrade(&base), world));
            }
        }
        self.forget_freed();
    }

    /// Keys of grids and trees nothing holds any more.
    fn forget_freed(&mut self) {
        self.grids.retain(|_, grid| grid.strong_count() > 0);
        self.sights.retain(|_, sight| sight.strong_count() > 0);
    }

    /// The maps other than `project` in memory that the store cannot drop:
    /// loading, or held by a match.
    fn held_except(&self, project: &str) -> Vec<String> {
        let mut held: Vec<String> = self
            .entries
            .iter()
            .filter(|(name, _)| name.as_str() != project)
            .filter(|(_, entry)| match entry {
                Entry::Loading => true,
                Entry::Ready { base, .. } => {
                    Arc::strong_count(base) > 1 || Arc::strong_count(&base.world) > 1
                }
                Entry::Dropped(base, world) => base.strong_count() > 0 || world.strong_count() > 0,
                Entry::Failed(_) => false,
            })
            .map(|(name, _)| name.clone())
            .collect();
        held.sort_unstable();
        held
    }

    /// Drops the ready maps other than `project` that no match holds, the
    /// least recently used first, till `room` of them are left; whether
    /// any was dropped.
    fn drop_idle(&mut self, project: &str, room: usize) -> bool {
        let mut idle: Vec<(u64, String)> = self
            .entries
            .iter()
            .filter_map(|(name, entry)| match entry {
                Entry::Ready { base, used }
                    if name.as_str() != project
                        && Arc::strong_count(base) == 1
                        && Arc::strong_count(&base.world) == 1 =>
                {
                    Some((*used, name.clone()))
                }
                _ => None,
            })
            .collect();
        idle.sort_unstable();
        let over = idle.len().saturating_sub(room);
        for (_, name) in idle.into_iter().take(over) {
            if let Some(Entry::Ready { base, .. }) = self.entries.remove(&name) {
                log::info!("Bot map {name} dropped from the store to make room for {project}");
                let world = Arc::downgrade(&base.world);
                self.entries
                    .insert(name, Entry::Dropped(Arc::downgrade(&base), world));
            }
        }
        self.forget_freed();
        over > 0
    }
}

/// Where a grid or tree came from, for the log.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Source {
    /// Another project of the same geometry has it.
    Shared,
    /// Read from its built file.
    Read,
    /// Built now (and written, if the directory takes it).
    Built,
}

/// How a map was made, for the log.
struct Built {
    seconds: f32,
    grid: Source,
    sight: Source,
    tactics: Source,
}

impl Built {
    fn how(&self) -> String {
        let word = |source: Source| match source {
            Source::Shared => "shared",
            Source::Read => "read",
            Source::Built => "built",
        };
        format!(
            "grid {}, sight tree {}, tactics {}",
            word(self.grid),
            word(self.sight),
            word(self.tactics)
        )
    }
}

/// The structure of `key` from its file in `built` if that reads back,
/// else made by `build` and written there for the next time.
fn read_or_build<T>(
    built: Option<&Path>,
    file_name: String,
    read: impl FnOnce(&Path) -> Result<T, mapdata::Error>,
    build: impl FnOnce() -> T,
    write: impl FnOnce(&T, &Path) -> Result<(), mapdata::Error>,
) -> (T, Source) {
    let Some(built) = built else {
        return (build(), Source::Built);
    };
    let path = built.join(file_name);
    if path.is_file() {
        match read(&path) {
            Ok(value) => return (value, Source::Read),
            Err(why) => log::warn!("Bot map file refused ({why}); built again"),
        }
    }
    let value = build();
    let written = std::fs::create_dir_all(built)
        .map_err(|e| mapdata::Error(format!("{}: {e}", built.display())))
        .and_then(|()| write(&value, &path));
    if let Err(why) = written {
        log::warn!("Bot map file not written ({why}); built at each load");
    }
    (value, Source::Built)
}

/// Reads one map, takes or builds its grid and tree, and builds its
/// tactics for the default margin.
fn build(
    path: &Path,
    built: Option<&Path>,
    cache: &Mutex<Cache>,
) -> Result<(TacticsBase, Built), String> {
    let started = Instant::now();
    let bytes = std::fs::read(path).map_err(|error| format!("{}: {error}", path.display()))?;
    let mut map_file = mapdata::digest::Digest::new();
    map_file.bytes(&bytes);
    let map_file = map_file.finish();
    let mut map =
        MapData::from_bytes(&bytes).map_err(|error| format!("{}: {error}", path.display()))?;
    drop(bytes);
    let built = catch_unwind(AssertUnwindSafe(|| {
        let (cell, body) = (mapdata::CELL, super::tuning::Tuning::default().grid_body());
        let walker = map.walker_triangles();
        let area = map.play_area(mapdata::AREA_MARGIN);
        let seeds = map.grid_seeds();
        let spawns: Vec<mapdata::Vec3> = map.spawns.iter().map(|s| s.position).collect();
        // With the gap links when the code default has them
        // (`GridTuning::gap_links`; the live server's tuning is the default).
        let links = super::tuning::Tuning::default().off_mesh_links();
        let grid_key = mapdata::grid_key_with_links(&walker, area, cell, body, &seeds, links);
        let shared = lock(cache).grids.get(&grid_key).and_then(Weak::upgrade);
        let (grid, grid_source) = match shared {
            Some(grid) => (grid, Source::Shared),
            None => {
                let (grid, source) = read_or_build(
                    built,
                    mapdata::built::grid_file_name(grid_key),
                    |path| WalkGrid::read(path, grid_key),
                    || WalkGrid::build_kept_with_links(&walker, area, cell, body, &seeds, &spawns, links).0,
                    |grid, path| grid.write(path, grid_key),
                );
                let grid = Arc::new(grid);
                lock(cache).grids.insert(grid_key, Arc::downgrade(&grid));
                (grid, source)
            }
        };
        drop(walker);
        let bullet = map.bullet_triangles();
        let sight_key = mapdata::sight_key(&bullet);
        let shared = lock(cache).sights.get(&sight_key).and_then(Weak::upgrade);
        let (sight, sight_source) = match shared {
            Some(sight) => (sight, Source::Shared),
            None => {
                let (sight, source) = read_or_build(
                    built,
                    mapdata::built::sight_file_name(sight_key),
                    |path| RayTree::read(path, sight_key),
                    || RayTree::build(&bullet),
                    |sight, path| sight.write(path, sight_key),
                );
                let sight = Arc::new(sight);
                lock(cache).sights.insert(sight_key, Arc::downgrade(&sight));
                (sight, source)
            }
        };
        drop(bullet);
        // The meshes are not read once the grid and the tree are built.
        map.libraries = Vec::new();
        map.placements = Vec::new();
        let world = Arc::new(World::assemble(map, grid, sight));
        let margin = Maps::margin();
        let key = TacticsBase::key(map_file, grid_key, margin);
        let (base, tactics_source) = read_or_build(
            built,
            mapdata::built::tactics_file_name(key),
            |path| TacticsBase::read(path, key, world.clone(), margin),
            || TacticsBase::build(world.clone(), margin),
            |base, path| base.write(path, key),
        );
        (base, grid_source, sight_source, tactics_source)
    }))
    .map_err(|_| format!("{}: the build panicked", path.display()))?;
    let (base, grid, sight, tactics) = built;
    Ok((
        base,
        Built {
            seconds: started.elapsed().as_secs_f32(),
            grid,
            sight,
            tactics,
        },
    ))
}

/// Bytes a built world holds: the grid, the sight tree and what is left of
/// the map's meshes (the objectives are a few kB).
pub fn world_bytes(world: &World) -> usize {
    let meshes: usize = world
        .map
        .libraries
        .iter()
        .map(|library| {
            library.name.len()
                + library.vertices.len() * size_of::<mapdata::Vec3>()
                + library.triangles.len() * size_of::<[u32; 3]>()
                + library.walker.len() * size_of::<mapdata::WalkerBehaviour>()
        })
        .sum();
    world.grid.memory_bytes()
        + world.sight.memory_bytes()
        + meshes
        + world.map.placements.len() * size_of::<mapdata::Placement>()
}

fn lock(cache: &Mutex<Cache>) -> MutexGuard<'_, Cache> {
    cache
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

impl fmt::Debug for Maps {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let cache = lock(&self.cache);
        let mut ready: Vec<&str> = cache
            .entries
            .iter()
            .filter(|(_, entry)| matches!(entry, Entry::Ready { .. }))
            .map(|(name, _)| name.as_str())
            .collect();
        ready.sort_unstable();
        f.debug_struct("Maps")
            .field("dir", &self.dir)
            .field("built", &self.built)
            .field("keep", &self.keep)
            .field("at_once", &self.at_once)
            .field("ready", &ready)
            .finish()
    }
}

/// One store equals only itself (`Settings` compares its fields).
impl PartialEq for Maps {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self, other)
    }
}

impl Eq for Maps {}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use mapdata::{Library, Placement, Spawn, Vec3, WalkerBehaviour};

    use super::*;

    /// A 20 × 20 m floor that walkers and bullets both meet, with a spawn
    /// for each team.
    fn tiny_map(project: &str) -> MapData {
        let floor = Library {
            name: "floor".into(),
            vertices: vec![
                Vec3::new(-10.0, 0.0, -10.0),
                Vec3::new(10.0, 0.0, -10.0),
                Vec3::new(10.0, 0.0, 10.0),
                Vec3::new(-10.0, 0.0, 10.0),
            ],
            triangles: vec![[0, 2, 1], [0, 3, 2]],
            walker: vec![WalkerBehaviour::Walk; 2],
            primitives: 0,
        };
        let spawn = |team: i32, x: f32| Spawn {
            position: Vec3::new(x, 0.0, 0.0),
            rotation: Vec3::ZERO,
            team,
            priority: 0,
            point_id: team,
            layer: "default".into(),
        };
        MapData {
            project: project.into(),
            levelmap_min: Vec3::new(-10.0, 0.0, -10.0),
            levelmap_max: Vec3::new(10.0, 1.0, 10.0),
            spawns: vec![spawn(0, -5.0), spawn(1, 5.0)],
            libraries: vec![floor],
            placements: vec![Placement {
                library: 0,
                position: Vec3::ZERO,
                rotation: Vec3::ZERO,
                cgroup: 10,
                cmask: 20,
            }],
            ..MapData::default()
        }
    }

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("srp-maps-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn wait_ready(maps: &Maps, project: &str, limit: Duration) -> Arc<TacticsBase> {
        let started = Instant::now();
        loop {
            match maps.get(project) {
                MapState::Ready(base) => return base,
                MapState::Loading if started.elapsed() < limit => {
                    std::thread::sleep(Duration::from_millis(5))
                }
                other => panic!("{project}: {other:?} after {:?}", started.elapsed()),
            }
        }
    }

    #[test]
    fn no_directory_means_no_map() {
        let maps = Maps::new(None);
        maps.request("level_01/day_4");
        assert!(matches!(maps.get("level_01/day_4"), MapState::Absent(_)));
    }

    #[test]
    fn a_missing_file_is_absent_with_its_name() {
        let dir = temp_dir("missing");
        let maps = Maps::new(Some(dir.clone()));
        let MapState::Absent(reason) = maps.get("level_09/day_4") else {
            panic!("a missing file must be absent");
        };
        assert!(reason.contains("level_09_day_4.mapdata"), "{reason}");
        // Remembered: still absent, nothing started.
        assert!(matches!(maps.get("level_09/day_4"), MapState::Absent(_)));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_damaged_file_is_absent() {
        let dir = temp_dir("damaged");
        std::fs::write(dir.join("broken_day_4.mapdata"), b"SRPMAPD\0nonsense").unwrap();
        let maps = Maps::new(Some(dir.clone()));
        let started = Instant::now();
        let reason = loop {
            match maps.get("broken/day_4") {
                MapState::Absent(reason) => break reason,
                MapState::Loading if started.elapsed() < Duration::from_secs(10) => {
                    std::thread::sleep(Duration::from_millis(5))
                }
                other => panic!("{other:?}"),
            }
        };
        assert!(reason.contains("broken_day_4.mapdata"), "{reason}");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_map_loads_in_the_background_and_is_shared() {
        let dir = temp_dir("tiny");
        let project = "tiny/day_4";
        tiny_map(project)
            .write(&dir.join(mapdata::format::file_name(project)))
            .unwrap();
        let maps = Maps::new(Some(dir.clone()));
        maps.request(project);
        // The build is on its own thread: the first look finds it loading
        // or (on a fast machine, rarely) done.
        assert!(matches!(
            maps.get(project),
            MapState::Loading | MapState::Ready(_)
        ));
        let base = wait_ready(&maps, project, Duration::from_secs(10));
        let world = base.world.clone();
        assert_eq!(world.map.project, project);
        assert_eq!(base.margin(), Maps::margin());
        // The meshes went once the grid and the tree were built.
        assert!(world.map.libraries.is_empty() && world.map.placements.is_empty());
        let a = world.grid.locate(Vec3::new(-5.0, 0.0, 0.0), 1.0).unwrap();
        let b = world.grid.locate(Vec3::new(5.0, 0.0, 0.0), 1.0).unwrap();
        assert!(world.grid.path(a, b).is_some());
        let MapState::Ready(again) = maps.get(project) else {
            panic!("a built world stays ready");
        };
        assert!(Arc::ptr_eq(&base, &again));
        assert!(format!("{maps:?}").contains(project));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_built_grid_and_tree_are_written_and_read_by_the_next_store() {
        let dir = temp_dir("built");
        let project = "tiny/day_4";
        tiny_map(project)
            .write(&dir.join(mapdata::format::file_name(project)))
            .unwrap();
        let wait = Duration::from_secs(10);
        let first = wait_ready(&Maps::new(Some(dir.clone())), project, wait);
        let files: Vec<String> = std::fs::read_dir(dir.join(BUILT_DIR))
            .unwrap()
            .map(|entry| entry.unwrap().file_name().into_string().unwrap())
            .collect();
        assert_eq!(files.len(), 3, "{files:?}");
        assert!(files.iter().any(|name| name.starts_with("grid-")));
        assert!(files.iter().any(|name| name.starts_with("sight-")));
        assert!(files.iter().any(|name| name.starts_with("tactics-")));
        // A new store (a restart) reads them: the same grid and tree.
        let second = wait_ready(&Maps::new(Some(dir.clone())), project, wait);
        assert!(!Arc::ptr_eq(&first.world.grid, &second.world.grid));
        assert_eq!(first.world.grid.digest(), second.world.grid.digest());
        assert_eq!(first.world.sight.digest(), second.world.sight.digest());
        assert_eq!(first.digest(), second.digest());
        // A damaged file is built again and written over.
        for name in &files {
            let path = dir.join(BUILT_DIR).join(name);
            let mut bytes = std::fs::read(&path).unwrap();
            let middle = bytes.len() / 2;
            bytes[middle] ^= 0x55;
            std::fs::write(&path, bytes).unwrap();
        }
        let third = wait_ready(&Maps::new(Some(dir.clone())), project, wait);
        assert_eq!(first.digest(), third.digest());
        for name in &files {
            let path = dir.join(BUILT_DIR).join(name);
            let key = u64::from_str_radix(&name[name.len() - 20..name.len() - 4], 16).unwrap();
            let fine = if name.starts_with("grid-") {
                WalkGrid::read(&path, key).is_ok()
            } else if name.starts_with("sight-") {
                RayTree::read(&path, key).is_ok()
            } else {
                TacticsBase::read(&path, key, third.world.clone(), Maps::margin()).is_ok()
            };
            assert!(fine, "{name} written over");
        }
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn projects_of_one_geometry_share_their_grid_and_tree() {
        let dir = temp_dir("shared");
        let write = |project: &str, map: MapData| {
            map.write(&dir.join(mapdata::format::file_name(project)))
                .unwrap();
        };
        write("tiny/day_4", tiny_map("tiny/day_4"));
        write("tiny/rain_4", tiny_map("tiny/rain_4"));
        // Another floor height: another grid and tree.
        let mut raised = tiny_map("tiny/evening_4");
        raised.placements[0].position = Vec3::new(0.0, 0.5, 0.0);
        write("tiny/evening_4", raised);
        let maps = Maps::with_keep(Some(dir.clone()), 3);
        let wait = Duration::from_secs(10);
        let day = wait_ready(&maps, "tiny/day_4", wait);
        let rain = wait_ready(&maps, "tiny/rain_4", wait);
        let evening = wait_ready(&maps, "tiny/evening_4", wait);
        assert!(
            !Arc::ptr_eq(&day, &rain),
            "each project has its own tactics"
        );
        assert_eq!(rain.world.map.project, "tiny/rain_4");
        assert!(Arc::ptr_eq(&day.world.grid, &rain.world.grid));
        assert!(Arc::ptr_eq(&day.world.sight, &rain.world.sight));
        assert!(!Arc::ptr_eq(&day.world.grid, &evening.world.grid));
        assert!(!Arc::ptr_eq(&day.world.sight, &evening.world.sight));
        // A shared grid is the grid the project builds on its own.
        let alone = World::build(tiny_map("tiny/rain_4"), mapdata::CELL, mapdata::Body::default());
        assert_eq!(rain.world.grid.digest(), alone.grid.digest());
        assert_eq!(rain.world.sight.digest(), alone.sight.digest());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn the_least_recently_used_world_is_dropped_but_kept_while_held() {
        let dir = temp_dir("lru");
        let projects = ["a/day_4", "b/day_4", "c/day_4"];
        for project in projects {
            tiny_map(project)
                .write(&dir.join(mapdata::format::file_name(project)))
                .unwrap();
        }
        let maps = Maps::with_keep(Some(dir.clone()), 2);
        let wait = Duration::from_secs(10);
        let a = wait_ready(&maps, "a/day_4", wait);
        let b = wait_ready(&maps, "b/day_4", wait);
        drop(b);
        // a used again after b: b is the oldest when c comes.
        wait_ready(&maps, "a/day_4", wait);
        wait_ready(&maps, "c/day_4", wait);
        let cache = lock(&maps.cache);
        assert!(matches!(cache.entries["a/day_4"], Entry::Ready { .. }));
        assert!(matches!(cache.entries["c/day_4"], Entry::Ready { .. }));
        let Entry::Dropped(b, _) = &cache.entries["b/day_4"] else {
            panic!("b must be dropped");
        };
        assert!(b.upgrade().is_none(), "nothing holds b");
        drop(cache);
        // a dropped while held comes back without a build.
        wait_ready(&maps, "b/day_4", wait);
        let cache = lock(&maps.cache);
        let Entry::Dropped(dropped, _) = &cache.entries["a/day_4"] else {
            panic!("a must be dropped now");
        };
        assert!(Arc::ptr_eq(&dropped.upgrade().unwrap(), &a));
        drop(cache);
        let MapState::Ready(back) = maps.get("a/day_4") else {
            panic!("a is held, so it is ready at once");
        };
        assert!(Arc::ptr_eq(&back, &a));
        let _ = std::fs::remove_dir_all(dir);
    }

    /// One map in memory at a time: a match on another map waits while
    /// one holds the first (its tactics or only its world), and the map
    /// no match holds any more is dropped to make room; a match on the map
    /// in memory has it at once.
    #[test]
    fn one_map_at_a_time_waits_for_room_and_drops_the_idle_one() {
        let dir = temp_dir("at-once");
        for project in ["a/day_4", "b/day_4", "c/day_4"] {
            tiny_map(project)
                .write(&dir.join(mapdata::format::file_name(project)))
                .unwrap();
        }
        let maps = Maps::configured(Some(dir.clone()), 1, 1, None);
        let wait = Duration::from_secs(10);
        let a = wait_ready(&maps, "a/day_4", wait);
        // A match holds a: b waits, and so does it again.
        for _ in 0..2 {
            let MapState::Waiting(why) = maps.get("b/day_4") else {
                panic!("b must wait while a is held");
            };
            assert!(why.contains("a/day_4"), "{why}");
        }
        // Another match on a has it at once.
        let MapState::Ready(again) = maps.get("a/day_4") else {
            panic!("a is in memory");
        };
        assert!(Arc::ptr_eq(&again, &a));
        drop(again);
        // Held by its world only (tactics of a match's own margin): still
        // held.
        let world = a.world.clone();
        drop(a);
        assert!(matches!(maps.get("b/day_4"), MapState::Waiting(_)));
        drop(world);
        // a is free: dropped for b, and nothing holds it.
        let b = wait_ready(&maps, "b/day_4", wait);
        {
            let cache = lock(&maps.cache);
            let Entry::Dropped(a, world) = &cache.entries["a/day_4"] else {
                panic!("a must be dropped for b");
            };
            assert!(a.upgrade().is_none() && world.upgrade().is_none());
        }
        // b is kept after its match: c drops it, it is not waited for.
        drop(b);
        wait_ready(&maps, "c/day_4", wait);
        let cache = lock(&maps.cache);
        assert!(matches!(cache.entries["b/day_4"], Entry::Dropped(..)));
        assert!(matches!(cache.entries["c/day_4"], Entry::Ready { .. }));
        drop(cache);
        let _ = std::fs::remove_dir_all(dir);
    }

    /// "Player vs Bots" (`SRP_PVB=on`): as many maps in memory as matches
    /// against bots may run (`SRP_PVB_MAX_MATCHES`, 5): five matches on five
    /// maps each have theirs at once, a sixth map waits, and gets in once a
    /// match lets its map go.
    #[test]
    fn five_matches_against_bots_hold_five_maps_at_once() {
        let dir = temp_dir("pvb-five");
        let projects = [
            "a/day_4", "b/day_4", "c/day_4", "d/day_4", "e/day_4", "f/day_4",
        ];
        for project in projects {
            tiny_map(project)
                .write(&dir.join(mapdata::format::file_name(project)))
                .unwrap();
        }
        let at_once = session::v026g::pvb::maps_at_once(AT_ONCE, true, 5);
        assert_eq!(at_once, 5);
        let maps = Maps::configured(Some(dir.clone()), KEEP, at_once, None);
        let wait = Duration::from_secs(10);
        let mut held: Vec<Arc<TacticsBase>> = projects[..5]
            .iter()
            .map(|project| wait_ready(&maps, project, wait))
            .collect();
        // All five still there, each its own.
        for (project, base) in projects.iter().zip(&held) {
            let MapState::Ready(again) = maps.get(project) else {
                panic!("{project} is in memory");
            };
            assert!(Arc::ptr_eq(&again, base));
        }
        let MapState::Waiting(why) = maps.get("f/day_4") else {
            panic!("a sixth map waits while five are held");
        };
        assert!(why.contains("at most 5"), "{why}");
        // One match is over: the sixth gets its map.
        held.remove(0);
        wait_ready(&maps, "f/day_4", wait);
        drop(held);
        let _ = std::fs::remove_dir_all(dir);
    }

    /// The memory of the bots' maps of "Player vs Bots" held at once, as
    /// matches on as many maps hold them (the owner's 2 GB host; all eight
    /// levels, of which five at most at once with `SRP_PVB_MAX_MATCHES` 5):
    /// `SRP_MAPDATA_DIR=<dir> cargo test -p match-server --release
    /// pvb_maps_held_at_once -- --ignored --nocapture` (the built files of
    /// `<dir>/built` are read, as the server reads them). Each map's world
    /// and tactics as counted, and on Linux the process's resident memory.
    #[test]
    #[ignore = "a measurement; needs SRP_MAPDATA_DIR"]
    fn pvb_maps_held_at_once() {
        let Some(dir) = real_dir() else { return };
        let built = Some(dir.join(BUILT_DIR));
        let mb = |bytes: usize| bytes as f64 / f64::from(1 << 20);
        let resident = || {
            let status = std::fs::read_to_string("/proc/self/status").ok()?;
            let line = status.lines().find(|line| line.starts_with("VmRSS:"))?;
            let kb: f64 = line.split_whitespace().nth(1)?.parse().ok()?;
            Some(kb / 1024.0)
        };
        let show = |value: Option<f64>| value.map_or("-".to_owned(), |mb| format!("{mb:.0} MB"));
        // The battery day of each level: what the lobby offers (its TDM
        // days are the same projects).
        let mut projects: Vec<&str> = [61_u8, 67, 73, 79, 85, 91, 97, 103]
            .iter()
            .filter_map(|&id| survarium::v026g::maps::map_mode(id))
            .map(|map| map.project)
            .collect();
        projects.dedup();
        let maps = Maps::configured(
            Some(dir),
            KEEP,
            session::v026g::pvb::maps_at_once(AT_ONCE, true, projects.len()),
            built,
        );
        let before = resident();
        eprintln!("before any map: {}", show(before));
        let mut held = Vec::new();
        let mut counted = 0;
        for project in &projects {
            let base = wait_ready(&maps, project, Duration::from_secs(300));
            mapdata::trim_freed_memory();
            let bytes = world_bytes(&base.world) + base.memory_bytes();
            counted += bytes;
            held.push(base);
            eprintln!(
                "{project}: {:.0} MB counted (world and tactics); held {}: {:.0} MB counted, \
                 resident {}",
                mb(bytes),
                held.len(),
                mb(counted),
                show(resident())
            );
        }
        if let (Some(before), Some(after)) = (before, resident()) {
            eprintln!(
                "{} maps held: {:.0} MB resident more, {:.0} MB a map",
                held.len(),
                after - before,
                (after - before) / held.len() as f64
            );
        }
        assert_eq!(held.len(), projects.len());
    }

    /// The directory of real maps, if the environment names one.
    fn real_dir() -> Option<PathBuf> {
        let dir = std::env::var_os("SRP_MAPDATA_DIR").map(PathBuf::from);
        if dir.is_none() {
            eprintln!("skipping: SRP_MAPDATA_DIR is not set");
        }
        dir
    }

    /// One real map, only with `SRP_MAPDATA_DIR=<dir of mapdata-dump files>`.
    #[test]
    fn a_real_map_loads() {
        let Some(dir) = real_dir() else { return };
        let maps = Maps::new(Some(dir));
        let world = wait_ready(&maps, "level_01/day_4", Duration::from_secs(300))
            .world
            .clone();
        assert_eq!(world.map.project, "level_01/day_4");
        for spawn in &world.map.spawns {
            assert!(world.grid.locate(spawn.position, 1.0).is_some());
        }
    }

    /// Every real map through two stores, only with `SRP_MAPDATA_DIR` and
    /// `SRP_BUILT_DIR` (an empty folder): the first builds and writes the
    /// grids, trees and tactics, the second (a restart) reads them; each
    /// map's digests are the same both ways, and the second never builds.
    #[test]
    #[ignore = "a measurement; needs SRP_MAPDATA_DIR and SRP_BUILT_DIR"]
    fn real_maps_read_back_through_the_store() {
        let (Some(dir), Some(built)) = (
            real_dir(),
            std::env::var_os("SRP_BUILT_DIR").map(PathBuf::from),
        ) else {
            return;
        };
        let mut projects: Vec<String> = std::fs::read_dir(&dir)
            .unwrap()
            .filter_map(|entry| {
                let name = entry.unwrap().file_name().into_string().unwrap();
                let stem = name.strip_suffix(".mapdata")?;
                let (level, weather) = stem.split_at(stem.find('_').unwrap() + 3);
                Some(format!("{level}/{}", &weather[1..]))
            })
            .collect();
        projects.sort();
        let wait = Duration::from_secs(300);
        let digests = |maps: &Maps, project: &str| {
            let base = wait_ready(maps, project, wait);
            (
                base.world.grid.digest(),
                base.world.sight.digest(),
                base.digest(),
            )
        };
        let first = Maps::configured(Some(dir.clone()), 1, AT_ONCE, Some(built.clone()));
        let started = Instant::now();
        let written: Vec<_> = projects.iter().map(|p| digests(&first, p)).collect();
        let build_s = started.elapsed().as_secs_f32();
        let second = Maps::configured(Some(dir.clone()), 1, AT_ONCE, Some(built.clone()));
        let started = Instant::now();
        for (project, written) in projects.iter().zip(&written) {
            assert_eq!(&digests(&second, project), written, "{project}");
        }
        let read_s = started.elapsed().as_secs_f32();
        let on_disk: u64 = std::fs::read_dir(&built)
            .unwrap()
            .map(|entry| entry.unwrap().metadata().unwrap().len())
            .sum();
        eprintln!(
            "{} projects: built and written in {build_s:.1} s, read in {read_s:.1} s; {} files, {} MB",
            projects.len(),
            std::fs::read_dir(&built).unwrap().count(),
            on_disk >> 20
        );
    }

    /// Build time and memory of a few real maps (the numbers in the notes):
    /// `SRP_MAPDATA_DIR=<dir> cargo test -p match-server --release real_map_sizes
    /// -- --ignored --nocapture`.
    #[test]
    #[ignore = "a measurement; needs SRP_MAPDATA_DIR"]
    fn real_map_sizes() {
        let Some(dir) = real_dir() else { return };
        let maps = Maps::with_keep(Some(dir), 1);
        for project in [
            "level_01/day_4",
            "level_02/day_4",
            "level_03/day_4",
            "level_04/day_4",
            "level_05/day_4",
            "level_06/day_4",
            "level_07/day_4",
            "level_08/day_4",
        ] {
            let started = Instant::now();
            let base = wait_ready(&maps, project, Duration::from_secs(300));
            let world = &base.world;
            let spawns = world.map.spawns.len();
            let located = world
                .map
                .spawns
                .iter()
                .filter(|s| world.grid.locate(s.position, 1.0).is_some())
                .count();
            eprintln!(
                "{project}: ready in {:.2} s; grid {} MB, tree {} MB, world {} MB, tactics {} MB; \
                 {located}/{spawns} spawns on a floor",
                started.elapsed().as_secs_f32(),
                world.grid.memory_bytes() >> 20,
                world.sight.memory_bytes() >> 20,
                world_bytes(world) >> 20,
                base.memory_bytes() >> 20,
            );
            assert_eq!(located, spawns);
        }
    }
}
