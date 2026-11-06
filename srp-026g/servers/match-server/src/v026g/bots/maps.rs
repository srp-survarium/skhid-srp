//! The maps the server's bots path on (`SRP_MAPDATA_DIR`,
//! `docs/notes/player_bots_mapdata.md` §6): one [`mapdata::World`] and its
//! [`TacticsBase`] per project, built from `<dir>/<project>.mapdata` (the
//! file `mapdata-dump` writes) on a thread of its own. A build takes
//! 0.3-0.8 s and the match loop is one thread, so a caller never waits: it
//! asks early ([`Maps::request`]) and looks again each tick
//! ([`Maps::get`]) until the map is [`MapState::Ready`].
//!
//! Every match on a map shares its world, and every match on it with the same
//! hazard margin its tactics (A326: one copy per map and margin; the default
//! margin's tactics are also kept as a built file). Projects built from the same geometry (the weather
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
//! host has under 1 GB, so the store builds one map at a time and **keeps
//! no map that no running match uses** (the owner's rule, A326). A match
//! asks for its map by a [`MapClaim`] ([`Maps::claim`]): the store holds a
//! map it built only till the first claim on it takes it ([`MapClaim::get`]);
//! from then on only the matches hold it (the store keeps a `Weak`), and it
//! is freed with the last of them. A claim dropped (the match ended) lets go
//! of a map built for it and never taken, and when that was the map's last
//! holder the freed memory is handed back to the system
//! ([`mapdata::trim_freed_memory`]: after the match's bots and world are
//! gone, as [`MapClaim`] is the last thing a match drops). A file that is
//! missing or fails to load is reported once and stays absent until the
//! server restarts.
//!
//! At most [`AT_ONCE`] maps are in memory (`SRP_MAPDATA_AT_ONCE`; the host
//! has room for one: `player_bots_mapdata.md`, "On host-like limits"). A
//! match on a map in memory has it at once. One that needs another map
//! while the maps in memory are all held by matches (or loading) waits
//! ([`MapState::Waiting`]): its bots run the stage 0 script, as while a
//! map loads, and take the map when it is loaded.

use std::collections::HashMap;
use std::fmt;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard, Weak};
use std::time::Instant;

use mapdata::{MapData, RayTree, WalkGrid, World};

use super::map_files::MapFiles;
use super::tactics::TacticsBase;
use super::tuning::{GridTuning, Tuning};

/// How many maps may be in memory at once by default, held by matches or
/// loading.
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
    /// Its world and its tactics for the claim's hazard margin.
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

/// Where a build thread is, for the tests: a build that does not finish names the step it is in (a no-op
/// outside the tests).
#[cfg(test)]
mod stage {
    use std::collections::HashMap;
    use std::sync::Mutex;
    use std::thread::ThreadId;
    use std::time::{Duration, Instant};

    type Where = HashMap<ThreadId, (&'static str, &'static str, Instant)>;
    static NOW: Mutex<Option<Where>> = Mutex::new(None);

    fn put(step: Option<&'static str>, detail: &'static str) {
        let mut now = NOW.lock().unwrap_or_else(|p| p.into_inner());
        let id = std::thread::current().id();
        let step = step.unwrap_or_else(|| now.as_ref().and_then(|w| w.get(&id)).map_or("", |s| s.0));
        now.get_or_insert_with(HashMap::new).insert(id, (step, detail, Instant::now()));
    }

    /// The build thread begins `step`.
    pub fn set(step: &'static str) {
        put(Some(step), "");
    }

    /// It is in `detail` of its step (reading, building or writing a built file).
    pub fn detail(detail: &'static str) {
        put(None, detail);
    }

    /// The step a thread was last in, the detail and how long ago it began it.
    pub fn of(id: ThreadId) -> Option<(&'static str, &'static str, Duration)> {
        let now = NOW.lock().unwrap_or_else(|p| p.into_inner());
        now.as_ref()?.get(&id).map(|(step, detail, since)| (*step, *detail, since.elapsed()))
    }
}

#[cfg(not(test))]
mod stage {
    #[inline(always)]
    pub fn set(_step: &'static str) {}

    #[inline(always)]
    pub fn detail(_detail: &'static str) {}
}

/// The store of built maps, shared by every match of the server.
pub struct Maps {
    dir: Option<PathBuf>,
    /// Where built grids, trees and tactics are kept; `None`: built at
    /// each load.
    built: Option<PathBuf>,
    /// The grids' body and off-mesh links: the server's tuning (`SRP_BOT_TUNING`, read once at its start), which
    /// every match of the server runs on. A grid is the map's, shared by every match on it.
    grid: GridTuning,
    /// Whether the grids get their table of wall clearances ([`Tuning::needs_wall_clearance`] of the server's tuning).
    wall_clearance: bool,
    /// The support their table of open sides is made for, if they get one ([`Tuning::needs_open_sides`] of the server's tuning).
    open_sides: Option<f32>,
    /// At most this many maps in memory ([`AT_ONCE`]).
    at_once: usize,
    cache: Arc<Mutex<Cache>>,
    /// Held by the build under way: one at a time, for memory and the one
    /// core of the host.
    builds: Arc<Mutex<()>>,
    /// The maps' bushes, grass and human routes, one copy per map while a match holds it ([`MapFiles`]).
    files: MapFiles,
    /// The build threads, to say in a test what one that does not finish is doing.
    #[cfg(test)]
    builders: Mutex<HashMap<String, std::thread::JoinHandle<()>>>,
}

/// A hazard margin as the tactics are keyed by: its bits (the tuning's `hazard.margin_m`, an `f32`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct Margin(u32);

impl Margin {
    fn of(margin_m: f32) -> Self {
        Self(margin_m.to_bits())
    }

    fn metres(self) -> f32 {
        f32::from_bits(self.0)
    }
}

#[derive(Default)]
struct Cache {
    entries: HashMap<String, Entry>,
    /// The live claims by project and margin: a map built for a claim is held by the store till one takes it.
    claims: HashMap<(String, Margin), usize>,
    /// Every grid and tree built, by the key of what it is built from,
    /// while something holds it.
    grids: HashMap<u64, Weak<WalkGrid>>,
    sights: HashMap<u64, Weak<RayTree>>,
    /// By project: how the build that made its map made it.
    made: HashMap<String, Made>,
}

/// One project in the store.
enum Entry {
    /// Its world is being read or built (with the tactics of the margin that asked first).
    Loading,
    /// Its world was made: held by its tactics and the matches, never by the store itself.
    Loaded(Loaded),
    /// The file is missing or would not load: absent till a restart.
    Failed(String),
}

/// A project whose world was made.
struct Loaded {
    world: Weak<World>,
    /// What its tactics are keyed by ([`TacticsBase::key`]).
    map_file: u64,
    grid_key: u64,
    /// Its tactics by hazard margin.
    tactics: HashMap<Margin, Tactic>,
}

/// One margin's tactics of a loaded project.
enum Tactic {
    /// Being built on the project's world (held by the build).
    Loading,
    /// Built for a claim that has not taken it yet: the store holds it till one does, or the last claim on it goes.
    Fresh(Arc<TacticsBase>),
    /// Taken by matches: gone with the last of them.
    Held(Weak<TacticsBase>),
}

/// A match's ask for its map: what [`MapClaim::get`] answers, and (dropped) the end of the match's hold on the map. A
/// match drops its claim last (`game/driven.rs`: its last field), after its bots and world, so a map it was the last to
/// hold is gone by then and its memory handed back.
pub struct MapClaim {
    maps: Arc<Maps>,
    project: String,
    margin: Margin,
}

impl MapClaim {
    /// Where the claimed map stands; starts its build (or its tactics' on a world in memory) as needed. Never blocks on
    /// a build.
    pub fn get(&self) -> MapState {
        self.maps.get(&self.project, self.margin)
    }

    pub fn project(&self) -> &str {
        &self.project
    }
}

impl Drop for MapClaim {
    fn drop(&mut self) {
        self.maps.unclaim(&self.project, self.margin);
    }
}

impl Maps {
    /// A store loading from `dir`; `None`: every project is absent. Its grids are the code defaults': for tests; the
    /// server's store is [`Maps::configured`] with the loaded tuning's.
    #[cfg(test)]
    pub fn new(dir: Option<PathBuf>) -> Arc<Self> {
        let built = dir.as_ref().map(|dir| dir.join(BUILT_DIR));
        Self::configured(dir, AT_ONCE, built, GridTuning::default())
    }

    /// As [`Maps::new`], with no bound on the maps in memory at once.
    #[cfg(test)]
    pub fn unbounded(dir: Option<PathBuf>) -> Arc<Self> {
        let built = dir.as_ref().map(|dir| dir.join(BUILT_DIR));
        Self::configured(dir, usize::MAX, built, GridTuning::default())
    }

    /// At most `at_once` maps in memory (at least one), the built files in `built` (none: built at each load), the grids
    /// made as `grid` says.
    pub fn configured(dir: Option<PathBuf>, at_once: usize, built: Option<PathBuf>, grid: GridTuning) -> Arc<Self> {
        Self::configured_for(dir, at_once, built, grid, false, None)
    }

    /// [`Maps::configured`], with the grids' table of wall clearances when `wall_clearance` and their table of open sides for the
    /// support `open_sides` (the server derives both from its tuning: [`Tuning::needs_wall_clearance`], [`Tuning::needs_open_sides`]).
    pub fn configured_for(
        dir: Option<PathBuf>,
        at_once: usize,
        built: Option<PathBuf>,
        grid: GridTuning,
        wall_clearance: bool,
        open_sides: Option<f32>,
    ) -> Arc<Self> {
        Arc::new(Self {
            dir,
            built,
            grid,
            wall_clearance,
            open_sides,
            at_once: at_once.max(1),
            cache: Arc::default(),
            builds: Arc::default(),
            files: MapFiles::default(),
            #[cfg(test)]
            builders: Mutex::default(),
        })
    }

    pub fn dir(&self) -> Option<&Path> {
        self.dir.as_deref()
    }

    /// The maps' bushes, grass and human routes, shared per map ([`MapFiles`]).
    pub fn files(&self) -> &MapFiles {
        &self.files
    }

    /// How `project`'s map was made by the build that made it, once one has.
    pub fn made(&self, project: &str) -> Option<Made> {
        lock(&self.cache).made.get(project).copied()
    }

    /// The hazard margin whose tactics are kept as built files: the tuning's default. Tactics for another margin are
    /// built at load, once per map and margin.
    pub fn margin() -> f32 {
        Tuning::default().hazard.margin_m.get()
    }

    /// A match's claim on `project`'s map (`level_01/day_4`) and its tactics for `margin_m`; its build starts now
    /// unless the map is built, being built or known to be absent.
    pub fn claim(self: &Arc<Self>, project: &str, margin_m: f32) -> MapClaim {
        let margin = Margin::of(margin_m);
        *lock(&self.cache).claims.entry((project.to_owned(), margin)).or_default() += 1;
        let claim = MapClaim { maps: self.clone(), project: project.to_owned(), margin };
        claim.get();
        claim
    }

    /// Where `project`'s map stands for `margin`; starts what it needs.
    fn get(&self, project: &str, margin: Margin) -> MapState {
        let Some(dir) = &self.dir else {
            return MapState::Absent("SRP_MAPDATA_DIR is not set".into());
        };
        let mut cache = lock(&self.cache);
        match cache.entries.get_mut(project) {
            Some(Entry::Loading) => return MapState::Loading,
            Some(Entry::Failed(reason)) => return MapState::Absent(reason.clone()),
            Some(Entry::Loaded(loaded)) => {
                match loaded.tactics.get_mut(&margin) {
                    Some(Tactic::Loading) => return MapState::Loading,
                    Some(tactic @ Tactic::Fresh(_)) => {
                        let Tactic::Fresh(base) = std::mem::replace(tactic, Tactic::Loading) else {
                            unreachable!("matched as fresh");
                        };
                        // Taken: from now on the matches hold it.
                        *tactic = Tactic::Held(Arc::downgrade(&base));
                        return MapState::Ready(base);
                    }
                    Some(Tactic::Held(base)) => {
                        if let Some(base) = base.upgrade() {
                            return MapState::Ready(base);
                        }
                    }
                    None => {}
                }
                if let Some(world) = loaded.world.upgrade() {
                    // The world is in memory (another margin's matches hold it): only this margin's tactics are made, on it.
                    let (map_file, grid_key) = (loaded.map_file, loaded.grid_key);
                    loaded.tactics.insert(margin, Tactic::Loading);
                    drop(cache);
                    self.spawn_tactics(project, world, map_file, grid_key, margin);
                    return MapState::Loading;
                }
                // Nothing holds the world any more: it is read again, below.
            }
            None => {}
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
        let path = dir.join(mapdata::format::file_name(project));
        if !path.is_file() {
            let reason = format!("no map file {}", path.display());
            log::warn!("Bot map {project}: {reason}; bots on it keep their script");
            cache.entries.insert(project.to_owned(), Entry::Failed(reason.clone()));
            return MapState::Absent(reason);
        }
        cache.entries.insert(project.to_owned(), Entry::Loading);
        drop(cache);
        self.spawn_build(project, path, margin);
        MapState::Loading
    }

    /// The end of a claim: a map built for its project and margin and never taken goes once no claim is left on it,
    /// and when nothing holds the project's world any more the freed memory is handed back.
    fn unclaim(&self, project: &str, margin: Margin) {
        let mut cache = lock(&self.cache);
        let key = (project.to_owned(), margin);
        let left = cache.claims.get_mut(&key).map(|count| {
            *count = count.saturating_sub(1);
            *count
        });
        if left == Some(0) {
            cache.claims.remove(&key);
            if let Some(Entry::Loaded(loaded)) = cache.entries.get_mut(project)
                && let Some(tactic @ Tactic::Fresh(_)) = loaded.tactics.get_mut(&margin)
                && let Tactic::Fresh(base) = std::mem::replace(tactic, Tactic::Loading)
            {
                *tactic = Tactic::Held(Arc::downgrade(&base));
                drop(base);
            }
        }
        if cache.unload_if_freed(project) {
            drop(cache);
            trim(project);
        }
    }

    fn spawn_build(&self, project: &str, path: PathBuf, margin: Margin) {
        let (cache, builds) = (self.cache.clone(), self.builds.clone());
        let name = project.to_owned();
        let (built, grid, wall_clearance, open_sides) = (self.built.clone(), self.grid.clone(), self.wall_clearance, self.open_sides);
        let spawned = std::thread::Builder::new()
            .name(format!("map {project}"))
            .spawn(move || {
                let result = {
                    let _turn = builds.lock().unwrap_or_else(|p| p.into_inner());
                    build(&path, built.as_deref(), &cache, &grid, wall_clearance, open_sides, margin)
                };
                stage::set("putting the map in the store");
                let mut cache = lock(&cache);
                match result {
                    Ok(made) => {
                        log::info!(
                            "Bot map {name} ready in {:.2} s ({}), {} MB with its tactics",
                            made.how.seconds,
                            made.how.how(),
                            (world_bytes(&made.base.world) + made.base.memory_bytes()) >> 20
                        );
                        cache.made.insert(name.clone(), made.how.made());
                        let world = Arc::downgrade(&made.base.world);
                        let mut tactics = HashMap::new();
                        tactics.insert(margin, cache.tactic(&name, margin, made.base));
                        cache.entries.insert(
                            name.clone(),
                            Entry::Loaded(Loaded { world, map_file: made.map_file, grid_key: made.grid_key, tactics }),
                        );
                        cache.forget_freed();
                        if cache.unload_if_freed(&name) {
                            drop(cache);
                            trim(&name);
                        }
                    }
                    Err(reason) => {
                        log::warn!("Bot map {name}: {reason}; bots on it keep their script");
                        cache.entries.insert(name, Entry::Failed(reason));
                    }
                }
            });
        #[cfg(test)]
        let spawned = spawned.map(|handle| {
            self.builders
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .insert(project.to_owned(), handle);
        });
        if let Err(error) = spawned {
            let reason = format!("can't start its build: {error}");
            log::warn!("Bot map {project}: {reason}");
            lock(&self.cache)
                .entries
                .insert(project.to_owned(), Entry::Failed(reason));
        }
    }

    /// Builds `margin`'s tactics on `project`'s world in memory, on a thread of its own (the build turn, as a map's).
    /// The default margin's are read from (or written to) the built files; another margin's are built.
    fn spawn_tactics(&self, project: &str, world: Arc<World>, map_file: u64, grid_key: u64, margin: Margin) {
        let (cache, builds) = (self.cache.clone(), self.builds.clone());
        let name = project.to_owned();
        let built = self.built.clone().filter(|_| margin == Margin::of(Self::margin()));
        let spawned = std::thread::Builder::new()
            .name(format!("tactics {project}"))
            .spawn(move || {
                let started = Instant::now();
                let result = {
                    let _turn = builds.lock().unwrap_or_else(|p| p.into_inner());
                    catch_unwind(AssertUnwindSafe(|| tactics_on(world, map_file, grid_key, margin, built.as_deref())))
                };
                let mut cache = lock(&cache);
                match result {
                    Ok((base, source)) => {
                        log::info!(
                            "Bot map {name}: tactics for margin {} {} in {:.2} s",
                            margin.metres(),
                            source.word(),
                            started.elapsed().as_secs_f32()
                        );
                        let tactic = cache.tactic(&name, margin, base);
                        if let Some(Entry::Loaded(loaded)) = cache.entries.get_mut(&name) {
                            loaded.tactics.insert(margin, tactic);
                        }
                        if cache.unload_if_freed(&name) {
                            drop(cache);
                            trim(&name);
                        }
                    }
                    Err(_) => {
                        let reason = format!("its tactics for margin {} panicked", margin.metres());
                        log::warn!("Bot map {name}: {reason}; bots on it keep their script");
                        cache.entries.insert(name, Entry::Failed(reason));
                    }
                }
            });
        if let Err(error) = spawned {
            let reason = format!("can't start its tactics build: {error}");
            log::warn!("Bot map {project}: {reason}");
            lock(&self.cache).entries.insert(project.to_owned(), Entry::Failed(reason));
        }
    }
}

impl Cache {
    /// What the store keeps of tactics just built: the tactics while a claim waits for them, else only a `Weak` (no
    /// match asks for them any more: they go now).
    fn tactic(&self, project: &str, margin: Margin, base: TacticsBase) -> Tactic {
        let base = Arc::new(base);
        if self.claims.get(&(project.to_owned(), margin)).is_some_and(|&count| count > 0) {
            Tactic::Fresh(base)
        } else {
            log::info!("Bot map {project}: built after its match ended; not kept");
            Tactic::Held(Arc::downgrade(&base))
        }
    }

    /// Forgets `project` if nothing holds its world any more (its last match ended, or it was built for none); whether
    /// it did.
    fn unload_if_freed(&mut self, project: &str) -> bool {
        let freed = matches!(self.entries.get(project), Some(Entry::Loaded(loaded)) if loaded.world.strong_count() == 0);
        if freed {
            self.entries.remove(project);
            self.forget_freed();
        }
        freed
    }

    /// Keys of grids and trees nothing holds any more.
    fn forget_freed(&mut self) {
        self.grids.retain(|_, grid| grid.strong_count() > 0);
        self.sights.retain(|_, sight| sight.strong_count() > 0);
    }

    /// The maps other than `project` in memory: loading, or a world something holds.
    fn held_except(&self, project: &str) -> Vec<String> {
        let mut held: Vec<String> = self
            .entries
            .iter()
            .filter(|(name, _)| name.as_str() != project)
            .filter(|(_, entry)| match entry {
                Entry::Loading => true,
                Entry::Loaded(loaded) => loaded.world.strong_count() > 0,
                Entry::Failed(_) => false,
            })
            .map(|(name, _)| name.clone())
            .collect();
        held.sort_unstable();
        held
    }
}

/// After a map's last holder let it go: what its world and tactics freed, back to the system (glibc keeps freed heap
/// blocks in the process till trimmed: `fx-review-a326-research.md`).
fn trim(project: &str) {
    log::info!("Bot map {project}: no match uses it; unloaded");
    mapdata::trim_freed_memory();
}

/// Where a grid, tree or tactics came from: for the log, and for the pre-build's check ([`prebuild`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
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

/// How a map's grid, sight tree and tactics were made by the build that made it ([`Maps::made`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Made {
    pub grid: Source,
    pub sight: Source,
    pub tactics: Source,
}

impl Made {
    /// Whether nothing of it was built: every part read from its file or shared with another project of the geometry.
    pub fn none_built(self) -> bool {
        [self.grid, self.sight, self.tactics].iter().all(|&source| source != Source::Built)
    }
}

impl Built {
    fn made(&self) -> Made {
        Made { grid: self.grid, sight: self.sight, tactics: self.tactics }
    }

    fn how(&self) -> String {
        format!("grid {}, sight tree {}, tactics {}", self.grid.word(), self.sight.word(), self.tactics.word())
    }
}

impl Source {
    fn word(self) -> &'static str {
        match self {
            Self::Shared => "shared",
            Self::Read => "read",
            Self::Built => "built",
        }
    }
}

/// A map one build made: its tactics (holding its world) for the margin it was built for, what tactics of its are keyed
/// by, and how it was made.
struct MadeMap {
    base: TacticsBase,
    map_file: u64,
    grid_key: u64,
    how: Built,
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
        stage::detail("building (no built files)");
        return (build(), Source::Built);
    };
    let path = built.join(file_name);
    if path.is_file() {
        stage::detail("reading the built file");
        match read(&path) {
            Ok(value) => return (value, Source::Read),
            Err(why) => log::warn!("Bot map file refused ({why}); built again"),
        }
    }
    stage::detail("building");
    let value = build();
    stage::detail("writing the built file");
    let written = std::fs::create_dir_all(built)
        .map_err(|e| mapdata::Error(format!("{}: {e}", built.display())))
        .and_then(|()| write(&value, &path));
    if let Err(why) = written {
        log::warn!("Bot map file not written ({why}); built at each load");
    }
    (value, Source::Built)
}

/// Reads one map, takes or builds its grid and tree, and builds its
/// tactics for `margin` (read from or written to the built files only for the default margin).
fn build(
    path: &Path,
    built: Option<&Path>,
    cache: &Mutex<Cache>,
    grid_tuning: &GridTuning,
    wall_clearance: bool,
    open_sides: Option<f32>,
    margin: Margin,
) -> Result<MadeMap, String> {
    let started = Instant::now();
    stage::set("reading the map file");
    let bytes = std::fs::read(path).map_err(|error| format!("{}: {error}", path.display()))?;
    stage::set("parsing the map file");
    let mut map_file = mapdata::digest::Digest::new();
    map_file.bytes(&bytes);
    let map_file = map_file.finish();
    let mut map =
        MapData::from_bytes(&bytes).map_err(|error| format!("{}: {error}", path.display()))?;
    drop(bytes);
    let built = catch_unwind(AssertUnwindSafe(|| {
        stage::set("the grid key");
        let (cell, body) = (mapdata::CELL, grid_tuning.body());
        let walker = map.walker_triangles();
        let area = map.play_area(mapdata::AREA_MARGIN);
        let seeds = map.grid_seeds();
        let spawns: Vec<mapdata::Vec3> = map.spawns.iter().map(|s| s.position).collect();
        // With the gap links when the server's tuning has them (`GridTuning::gap_links`).
        let links = grid_tuning.off_mesh_links();
        let grid_key = mapdata::grid_key_with_links(&walker, area, cell, body, &seeds, links);
        stage::set("the grid");
        let shared = lock(cache).grids.get(&grid_key).and_then(Weak::upgrade);
        let (grid, grid_source) = match shared {
            Some(grid) => (grid, Source::Shared),
            None => {
                let (grid, source) = read_or_build(
                    built,
                    mapdata::built::grid_file_name(grid_key),
                    |path| WalkGrid::read(path, grid_key, body),
                    || WalkGrid::build_kept_with_links(&walker, area, cell, body, &seeds, &spawns, links).0,
                    |grid, path| grid.write(path, grid_key),
                );
                // The table of wall clearances ([`WalkGrid::wall_clearance`]) only where the tuning in force needs it
                // ([`Tuning::needs_wall_clearance`]):
                // derived from the triangles, in neither the cached file nor the key, and 0.5-1.7 s of CPU a map.
                let grid = if wall_clearance { grid.with_wall_clearance(&walker) } else { grid };
                // The table of open sides ([`WalkGrid::open_toward`]) likewise ([`Tuning::needs_open_sides`]).
                let grid = match open_sides {
                    Some(support_m) => grid.with_open_sides(&walker, support_m),
                    None => grid,
                };
                let grid = Arc::new(grid);
                lock(cache).grids.insert(grid_key, Arc::downgrade(&grid));
                (grid, source)
            }
        };
        drop(walker);
        stage::set("the sight tree key");
        let bullet = map.bullet_mesh();
        let sight_key = bullet.key();
        stage::set("the sight tree");
        let shared = lock(cache).sights.get(&sight_key).and_then(Weak::upgrade);
        let (sight, sight_source) = match shared {
            Some(sight) => (sight, Source::Shared),
            None => {
                let (sight, source) = read_or_build(
                    built,
                    mapdata::built::sight_file_name(sight_key),
                    |path| RayTree::read(path, sight_key),
                    || bullet.build_tree(),
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
        stage::set("assembling the world");
        let world = Arc::new(World::assemble(map, grid, sight));
        stage::set("the tactics");
        let files = built.filter(|_| margin == Margin::of(Maps::margin()));
        let (base, tactics_source) = tactics_on(world, map_file, grid_key, margin, files);
        (base, grid_key, grid_source, sight_source, tactics_source)
    }))
    .map_err(|_| format!("{}: the build panicked", path.display()))?;
    let (base, grid_key, grid, sight, tactics) = built;
    Ok(MadeMap {
        base,
        map_file,
        grid_key,
        how: Built { seconds: started.elapsed().as_secs_f32(), grid, sight, tactics },
    })
}

/// `margin`'s tactics on `world`: from the built file in `built` when it reads back, else built (and written there).
fn tactics_on(world: Arc<World>, map_file: u64, grid_key: u64, margin: Margin, built: Option<&Path>) -> (TacticsBase, Source) {
    let margin = margin.metres();
    let key = TacticsBase::key(map_file, grid_key, margin);
    read_or_build(
        built,
        mapdata::built::tactics_file_name(key),
        |path| TacticsBase::read(path, key, world.clone(), margin),
        || TacticsBase::build(world.clone(), margin),
        |base, path| base.write(path, key),
    )
}

/// One map of a pre-build ([`prebuild`]): how the first store made it and in how long, and its digests (grid, sight tree,
/// tactics), which a second store read back the same.
#[derive(Clone, Debug)]
pub struct Prebuilt {
    pub project: String,
    pub made: Made,
    pub seconds: f32,
    pub digests: [u64; 3],
}

/// `project`'s map from `maps`, waiting up to `limit` for its build.
fn ready(maps: &Arc<Maps>, project: &str, limit: std::time::Duration) -> Result<Arc<TacticsBase>, String> {
    let started = Instant::now();
    let claim = maps.claim(project, Maps::margin());
    loop {
        match claim.get() {
            MapState::Ready(base) => return Ok(base),
            MapState::Absent(why) => return Err(format!("{project}: {why}")),
            MapState::Loading | MapState::Waiting(_) if started.elapsed() < limit => {
                std::thread::sleep(std::time::Duration::from_millis(20));
            }
            other => return Err(format!("{project}: {other:?} after {:?}", started.elapsed())),
        }
    }
}

/// The bots' maps of `projects` built ahead of a release (A197): a store from `store` (the server's own,
/// [`crate::v026g::bot_maps`], so the files have the keys the server computes) makes each map, one at a time, and writes
/// its grid, sight tree and tactics to its built folder; then a second store from `store` (a restart) loads each again and
/// must read every part (or share it) with the same digests. Err names the first project a restart would build again, or
/// read otherwise: a key the server would not find fails here, not in the first live match on the map.
pub fn prebuild(store: impl Fn() -> Arc<Maps>, projects: &[String], limit: std::time::Duration) -> Result<Vec<Prebuilt>, String> {
    let digests = |base: &TacticsBase| [base.world.grid.digest(), base.world.sight.digest(), base.digest()];
    let first = store();
    let mut done = Vec::with_capacity(projects.len());
    for project in projects {
        let started = Instant::now();
        let base = ready(&first, project, limit)?;
        let made = first.made(project).ok_or_else(|| format!("{project}: ready with no build"))?;
        done.push(Prebuilt { project: project.clone(), made, seconds: started.elapsed().as_secs_f32(), digests: digests(&base) });
    }
    drop(first);
    let second = store();
    for map in &done {
        let base = ready(&second, &map.project, limit)?;
        let made = second.made(&map.project).ok_or_else(|| format!("{}: ready with no build", map.project))?;
        if !made.none_built() {
            return Err(format!("{}: a restart built it again ({made:?}): its files are not where the server looks", map.project));
        }
        if digests(&base) != map.digests {
            return Err(format!("{}: read back otherwise than built ({:?} vs {:?})", map.project, digests(&base), map.digests));
        }
    }
    Ok(done)
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
                + library.faces.len() * size_of::<mapdata::library::Face>()
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
        let mut loaded: Vec<&str> = cache
            .entries
            .iter()
            .filter(|(_, entry)| matches!(entry, Entry::Loaded(loaded) if loaded.world.strong_count() > 0))
            .map(|(name, _)| name.as_str())
            .collect();
        loaded.sort_unstable();
        f.debug_struct("Maps")
            .field("dir", &self.dir)
            .field("built", &self.built)
            .field("at_once", &self.at_once)
            .field("loaded", &loaded)
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
            faces: vec![
                mapdata::library::Face::unknown([0, 2, 1], WalkerBehaviour::Walk),
                mapdata::library::Face::unknown([0, 3, 2], WalkerBehaviour::Walk),
            ],
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

    /// What the build thread of `project` is doing, for a build that does not finish in time: whether it ended
    /// (and with what panic), else the step it is in and for how long.
    fn build_report(maps: &Maps, project: &str) -> String {
        let mut builders = maps.builders.lock().unwrap_or_else(|p| p.into_inner());
        let Some(handle) = builders.remove(project) else {
            return "no build thread was started for it".to_owned();
        };
        let id = handle.thread().id();
        let at = stage::of(id).map_or_else(
            || "no step recorded".to_owned(),
            |(step, detail, since)| format!("step \"{step}\" {detail} for {since:?}"),
        );
        if handle.is_finished() {
            return match handle.join() {
                Ok(()) => format!("the build thread ended without a result in the store ({at})"),
                Err(payload) => {
                    let why = payload
                        .downcast_ref::<&str>()
                        .map(|s| (*s).to_owned())
                        .or_else(|| payload.downcast_ref::<String>().cloned())
                        .unwrap_or_else(|| "a payload that is no text".to_owned());
                    format!("the build thread panicked: {why} ({at})")
                }
            };
        }
        let report = format!("the build thread is still running: {at}");
        builders.insert(project.to_owned(), handle);
        report
    }

    /// Where `project`'s map stands for the default margin, to a claim made and dropped at once (it starts no build:
    /// for absent, waiting and held maps).
    fn look(maps: &Arc<Maps>, project: &str) -> MapState {
        maps.claim(project, Maps::margin()).get()
    }

    /// `project`'s map once built, as a match takes it (its claim gone after, the map held by the caller).
    fn wait_ready(maps: &Arc<Maps>, project: &str, limit: Duration) -> Arc<TacticsBase> {
        let started = Instant::now();
        let claim = maps.claim(project, Maps::margin());
        loop {
            match claim.get() {
                MapState::Ready(base) => return base,
                MapState::Loading if started.elapsed() < limit => {
                    std::thread::sleep(Duration::from_millis(5))
                }
                other => panic!(
                    "{project}: {other:?} after {:?}; {}",
                    started.elapsed(),
                    build_report(maps, project)
                ),
            }
        }
    }

    /// The diagnosis of a build that does not finish names its thread and step.
    #[test]
    fn a_build_that_does_not_finish_in_time_is_reported_with_its_step() {
        let dir = temp_dir("report");
        let project = "tiny/day_4";
        tiny_map(project)
            .write(&dir.join(mapdata::format::file_name(project)))
            .unwrap();
        let maps = Maps::new(Some(dir.clone()));
        // Holding the build turn keeps the thread from starting its work: it waits, with no step yet.
        let turn = maps.builds.lock().unwrap();
        let claim = maps.claim(project, Maps::margin());
        assert!(matches!(claim.get(), MapState::Loading));
        std::thread::sleep(Duration::from_millis(50));
        let report = build_report(&maps, project);
        assert!(report.contains("still running"), "{report}");
        drop(turn);
        let _ = wait_ready(&maps, project, Duration::from_secs(10));
        drop(claim);
        // In the store: the thread ended cleanly, and its last step is on record.
        let report = build_report(&maps, project);
        assert!(report.contains("ended without a result"), "{report}");
        assert!(report.contains("putting the map in the store"), "{report}");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn no_directory_means_no_map() {
        let maps = Maps::new(None);
        assert!(matches!(look(&maps, "level_01/day_4"), MapState::Absent(_)));
    }

    #[test]
    fn a_missing_file_is_absent_with_its_name() {
        let dir = temp_dir("missing");
        let maps = Maps::new(Some(dir.clone()));
        let MapState::Absent(reason) = look(&maps, "level_09/day_4") else {
            panic!("a missing file must be absent");
        };
        assert!(reason.contains("level_09_day_4.mapdata"), "{reason}");
        // Remembered: still absent, nothing started.
        assert!(matches!(look(&maps, "level_09/day_4"), MapState::Absent(_)));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_damaged_file_is_absent() {
        let dir = temp_dir("damaged");
        std::fs::write(dir.join("broken_day_4.mapdata"), b"SRPMAPD\0nonsense").unwrap();
        let maps = Maps::new(Some(dir.clone()));
        let started = Instant::now();
        let claim = maps.claim("broken/day_4", Maps::margin());
        let reason = loop {
            match claim.get() {
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
        let claim = maps.claim(project, Maps::margin());
        // The build is on its own thread: the first look finds it loading
        // or (on a fast machine, rarely) done.
        assert!(matches!(
            claim.get(),
            MapState::Loading | MapState::Ready(_)
        ));
        let base = wait_ready(&maps, project, Duration::from_secs(10));
        drop(claim);
        let world = base.world.clone();
        assert_eq!(world.map.project, project);
        assert_eq!(base.margin(), Maps::margin());
        // The meshes went once the grid and the tree were built.
        assert!(world.map.libraries.is_empty() && world.map.placements.is_empty());
        let a = world.grid.locate(Vec3::new(-5.0, 0.0, 0.0), 1.0).unwrap();
        let b = world.grid.locate(Vec3::new(5.0, 0.0, 0.0), 1.0).unwrap();
        assert!(world.grid.path(a, b).is_some());
        let MapState::Ready(again) = look(&maps, project) else {
            panic!("a built world stays ready while held");
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
                WalkGrid::read(&path, key, GridTuning::default().body()).is_ok()
            } else if name.starts_with("sight-") {
                RayTree::read(&path, key).is_ok()
            } else {
                TacticsBase::read(&path, key, third.world.clone(), Maps::margin()).is_ok()
            };
            assert!(fine, "{name} written over");
        }
        let _ = std::fs::remove_dir_all(dir);
    }

    /// A197: the maps built ahead of a release (`srp-build-maps`: [`prebuild`] through [`crate::v026g::bot_maps`]) are what
    /// the server's own store (`parse_mapdata_dir`, as the server makes it from its settings) reads at load, grid, sight
    /// tree and tactics alike, with the same digests: their keys are the server's.
    #[test]
    fn a_prebuilt_map_is_read_by_the_servers_store() {
        let dir = temp_dir("prebuilt");
        let built = dir.join("built");
        let project = "tiny/day_4";
        tiny_map(project).write(&dir.join(mapdata::format::file_name(project))).unwrap();
        let tuning = crate::v026g::bots::tuning::Tuning::default();
        let store = || crate::v026g::bot_maps(dir.clone(), AT_ONCE, Some(built.clone()), &tuning);
        let done = prebuild(store, &[project.to_owned()], Duration::from_secs(10)).unwrap();
        assert_eq!(done.len(), 1);
        assert!(done[0].made.grid == Source::Built && done[0].made.sight == Source::Built && done[0].made.tactics == Source::Built);
        let server = crate::v026g::parse_mapdata_dir(
            dir.to_str(),
            AT_ONCE as u32,
            built.to_str(),
            &tuning,
        )
        .unwrap();
        let base = wait_ready(&server, project, Duration::from_secs(10));
        assert_eq!(server.made(project), Some(Made { grid: Source::Read, sight: Source::Read, tactics: Source::Read }));
        assert_eq!([base.world.grid.digest(), base.world.sight.digest(), base.digest()], done[0].digests);
        let _ = std::fs::remove_dir_all(dir);
    }

    /// A pre-build a restart would not read back fails the run (here: a store that keeps no files), naming the map.
    #[test]
    fn a_prebuild_whose_files_a_restart_does_not_find_fails() {
        let dir = temp_dir("prebuilt-none");
        let project = "tiny/day_4";
        tiny_map(project).write(&dir.join(mapdata::format::file_name(project))).unwrap();
        let store = || Maps::configured(Some(dir.clone()), AT_ONCE, None, GridTuning::default());
        let why = prebuild(store, &[project.to_owned()], Duration::from_secs(10)).unwrap_err();
        assert!(why.starts_with("tiny/day_4: a restart built it again"), "{why}");
        let _ = std::fs::remove_dir_all(dir);
    }

    /// The keys carry the tuning's grid: a server tuned otherwise than the pre-build does not find its grid and builds it
    /// again (what the pre-build's own read-back catches when it runs with the server's tuning).
    #[test]
    fn a_server_tuned_otherwise_builds_the_prebuilt_grid_again() {
        let dir = temp_dir("prebuilt-other");
        let built = dir.join("built");
        let project = "tiny/day_4";
        tiny_map(project).write(&dir.join(mapdata::format::file_name(project))).unwrap();
        let tuning = crate::v026g::bots::tuning::Tuning::default();
        let store = || crate::v026g::bot_maps(dir.clone(), AT_ONCE, Some(built.clone()), &tuning);
        prebuild(store, &[project.to_owned()], Duration::from_secs(10)).unwrap();
        let other = crate::v026g::bots::tuning::Tuning {
            grid: GridTuning { client_numbers: false, gap_links: false, ..GridTuning::default() },
            ..Default::default()
        };
        let server = crate::v026g::parse_mapdata_dir(dir.to_str(), AT_ONCE as u32, built.to_str(), &other).unwrap();
        wait_ready(&server, project, Duration::from_secs(10));
        assert_eq!(server.made(project).map(|made| made.grid), Some(Source::Built));
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
        let maps = Maps::unbounded(Some(dir.clone()));
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
        let defaults = crate::v026g::bots::tuning::Tuning::default();
        let alone = World::build_with_links(
            tiny_map("tiny/rain_4"),
            mapdata::CELL,
            defaults.grid_body(),
            defaults.off_mesh_links(),
        );
        assert_eq!(rain.world.grid.digest(), alone.grid.digest());
        assert_eq!(rain.world.sight.digest(), alone.sight.digest());
        let _ = std::fs::remove_dir_all(dir);
    }

    /// A tuning file's `grid` keys reach the server's grids (A137): the maps are built as the loaded tuning says, not
    /// as the code's defaults.
    #[test]
    fn the_grids_are_built_as_the_servers_tuning_says() {
        let dir = temp_dir("tuned-grid");
        tiny_map("tiny/day_4").write(&dir.join(mapdata::format::file_name("tiny/day_4"))).unwrap();
        let grid_tuning = GridTuning { client_numbers: false, gap_links: false, ..GridTuning::default() };
        assert_ne!(grid_tuning, GridTuning::default());
        let wait = Duration::from_secs(10);
        let tuned = wait_ready(
            &Maps::configured(Some(dir.clone()), AT_ONCE, None, grid_tuning.clone()),
            "tiny/day_4",
            wait,
        );
        let alone = World::build_with_links(
            tiny_map("tiny/day_4"),
            mapdata::CELL,
            grid_tuning.body(),
            grid_tuning.off_mesh_links(),
        );
        assert_eq!(tuned.world.grid.digest(), alone.grid.digest());
        let default = wait_ready(&Maps::configured(Some(dir.clone()), AT_ONCE, None, GridTuning::default()), "tiny/day_4", wait);
        assert_ne!(tuned.world.grid.digest(), default.world.grid.digest(), "the body is in the grid");
        let _ = std::fs::remove_dir_all(dir);
    }

    /// A326, the owner's rule: a map no running match uses is not kept. Held, a later match shares it; once its last
    /// match lets it go, the store holds nothing of it (no world, no grid or tree key), and the next match reads it again.
    #[test]
    fn a_map_no_match_uses_is_unloaded() {
        let dir = temp_dir("unload");
        let project = "a/day_4";
        tiny_map(project).write(&dir.join(mapdata::format::file_name(project))).unwrap();
        let maps = Maps::new(Some(dir.clone()));
        let wait = Duration::from_secs(10);
        let a = wait_ready(&maps, project, wait);
        let MapState::Ready(again) = look(&maps, project) else {
            panic!("a held map is ready at once");
        };
        assert!(Arc::ptr_eq(&a, &again));
        drop(again);
        let world = Arc::downgrade(&a.world);
        let claim = maps.claim(project, Maps::margin());
        drop(a);
        assert!(world.upgrade().is_none(), "the store holds no map nobody uses");
        drop(claim);
        {
            let cache = lock(&maps.cache);
            assert!(!cache.entries.contains_key(project), "forgotten once its last claim went");
            assert!(cache.grids.is_empty() && cache.sights.is_empty());
        }
        let b = wait_ready(&maps, project, wait);
        assert_eq!(b.world.map.project, project, "read again for the next match");
        let _ = std::fs::remove_dir_all(dir);
    }

    /// A map built for a match that ended before it was ready is not kept either.
    #[test]
    fn a_map_built_after_its_match_ended_is_not_kept() {
        let dir = temp_dir("orphan");
        let project = "a/day_4";
        tiny_map(project).write(&dir.join(mapdata::format::file_name(project))).unwrap();
        let maps = Maps::new(Some(dir.clone()));
        let turn = maps.builds.lock().unwrap();
        let claim = maps.claim(project, Maps::margin());
        assert!(matches!(claim.get(), MapState::Loading));
        drop(claim);
        drop(turn);
        let handle = maps.builders.lock().unwrap().remove(project).unwrap();
        handle.join().unwrap();
        let cache = lock(&maps.cache);
        assert!(!cache.entries.contains_key(project), "built for nobody: not kept");
        assert!(cache.grids.is_empty() && cache.sights.is_empty());
        drop(cache);
        let _ = std::fs::remove_dir_all(dir);
    }

    /// A326 items 3 and 6: matches tuned to another hazard margin share one copy of that margin's tactics, on the one
    /// world of the map (not read again while it is held); the default margin's and another's are not the same tactics.
    #[test]
    fn matches_with_one_margin_share_its_tactics_on_the_one_world() {
        let dir = temp_dir("margins");
        let project = "a/day_4";
        tiny_map(project).write(&dir.join(mapdata::format::file_name(project))).unwrap();
        let maps = Maps::new(Some(dir.clone()));
        let wait = Duration::from_secs(10);
        let ready = |margin: f32| {
            let claim = maps.claim(project, margin);
            let started = Instant::now();
            loop {
                match claim.get() {
                    MapState::Ready(base) => return base,
                    MapState::Loading if started.elapsed() < wait => std::thread::sleep(Duration::from_millis(5)),
                    other => panic!("{other:?}"),
                }
            }
        };
        let other = Maps::margin() + 0.5;
        let first = ready(other);
        let second = ready(other);
        assert!(Arc::ptr_eq(&first, &second), "one copy for the margin");
        assert_eq!(first.margin(), other);
        let default = ready(Maps::margin());
        assert!(Arc::ptr_eq(&default.world, &first.world), "the world held is not read again");
        assert!(!Arc::ptr_eq(&default, &first));
        assert_eq!(default.margin(), Maps::margin());
        // The margin's tactics are the ones a match would build on its own.
        assert_eq!(first.digest(), TacticsBase::build(first.world.clone(), other).digest());
        let _ = std::fs::remove_dir_all(dir);
    }

    /// One map in memory at a time: a match on another map waits while
    /// one holds the first (its tactics or only its world), and gets room
    /// once nothing holds it; a match on the map in memory has it at once.
    #[test]
    fn one_map_at_a_time_waits_for_room() {
        let dir = temp_dir("at-once");
        for project in ["a/day_4", "b/day_4", "c/day_4"] {
            tiny_map(project)
                .write(&dir.join(mapdata::format::file_name(project)))
                .unwrap();
        }
        let maps = Maps::configured(Some(dir.clone()), 1, None, GridTuning::default());
        let wait = Duration::from_secs(10);
        let a = wait_ready(&maps, "a/day_4", wait);
        // A match holds a: b waits, and so does it again.
        for _ in 0..2 {
            let MapState::Waiting(why) = look(&maps, "b/day_4") else {
                panic!("b must wait while a is held");
            };
            assert!(why.contains("a/day_4"), "{why}");
        }
        // Another match on a has it at once.
        let MapState::Ready(again) = look(&maps, "a/day_4") else {
            panic!("a is in memory");
        };
        assert!(Arc::ptr_eq(&again, &a));
        drop(again);
        // Held by its world only: still held.
        let world = a.world.clone();
        drop(a);
        assert!(matches!(look(&maps, "b/day_4"), MapState::Waiting(_)));
        drop(world);
        // a is free: b gets in, and nothing of a is left.
        let b = wait_ready(&maps, "b/day_4", wait);
        assert!(!lock(&maps.cache).entries.contains_key("a/day_4"));
        // b's match ends: c needs no room made.
        drop(b);
        wait_ready(&maps, "c/day_4", wait);
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
        let maps = Maps::configured(Some(dir.clone()), at_once, None, GridTuning::default());
        let wait = Duration::from_secs(10);
        let mut held: Vec<Arc<TacticsBase>> = projects[..5]
            .iter()
            .map(|project| wait_ready(&maps, project, wait))
            .collect();
        // All five still there, each its own.
        for (project, base) in projects.iter().zip(&held) {
            let MapState::Ready(again) = look(&maps, project) else {
                panic!("{project} is in memory");
            };
            assert!(Arc::ptr_eq(&again, base));
        }
        let MapState::Waiting(why) = look(&maps, "f/day_4") else {
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
            session::v026g::pvb::maps_at_once(AT_ONCE, true, projects.len()),
            built,
            GridTuning::default(),
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
        let first = Maps::configured(Some(dir.clone()), AT_ONCE, Some(built.clone()), GridTuning::default());
        let started = Instant::now();
        let written: Vec<_> = projects.iter().map(|p| digests(&first, p)).collect();
        let build_s = started.elapsed().as_secs_f32();
        let second = Maps::configured(Some(dir.clone()), AT_ONCE, Some(built.clone()), GridTuning::default());
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
        let maps = Maps::new(Some(dir));
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
