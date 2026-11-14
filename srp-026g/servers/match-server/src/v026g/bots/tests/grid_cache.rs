//! [bench speed] The bench's worlds from a cache of built grids and sight trees (`SRP_BENCH_GRID_CACHE=<dir>`): most of a bench
//! match's wall time was building its map's world (the grid's build, 31-1043 s a match against 8-40 s of play in 60 rev56 and v6
//! bench logs), the same world for every seed, arm and session. With the variable set, [`world`] reads the map's grid and sight
//! tree from `<dir>` by their keys and builds (and writes) a part only on a miss; unset, it is `World::build_with_extras`.
//!
//! What a file's key covers:
//! - the grid: the server's own key (`mapdata::grid_key_with_links`: the walker triangles as placed, the play area, the cell, the
//!   grid body, the seeds, the off-mesh links and the build's algorithm numbers), the spawns (`WalkGrid::build_kept_with_links`
//!   keeps their pockets; the server's key leaves them out because its maps share a grid only between projects of one geometry);
//! - the sight tree: the server's own key (`mapdata::sight_key`: the bullet triangles and their armor);
//! - both: the grid builder's code, as [`source_fingerprint`] (its file names carry it, so the files of another builder are never
//!   read): every file of the `mapdata` crate, which holds the grid's and the tree's builds and everything they call (`survarium`,
//!   `resources-db` and `binary-config-parser` are called only while the map file is read, whose output the keys hold, and by
//!   `Armor::crossed_by`, a query), the workspace's `Cargo.toml` (the profiles) and `Cargo.lock` (the dependencies' versions), and
//!   `flake.nix` with `flake.lock` (the toolchain).
//!
//! The tables `mapdata::GridExtras` adds are not cached: they are made from the read grid as the server's map store makes them
//! (`bots::maps`), by the one `GridExtras::apply`. A file is written to a name of its own and renamed in, so a reader never sees
//! half of one and two sessions writing the same key never mix; the runs that miss a key together build it once (its lock file); a
//! file that does not read back (the key, the digest) is built again.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::Instant;

use mapdata::digest::Digest;
use mapdata::{Body, GridExtras, MapData, OffMeshLinks, RayTree, WalkGrid, World};

use super::settings::bench_settings;

/// The cache of this run (`SRP_BENCH_GRID_CACHE`), or none.
struct Cache {
    dir: PathBuf,
    /// [`source_fingerprint`] of the source this test binary was built from.
    source: u64,
}

fn cache() -> Option<&'static Cache> {
    static CACHE: OnceLock<Option<Cache>> = OnceLock::new();
    CACHE
        .get_or_init(|| {
            let dir = PathBuf::from(bench_settings().var_os("SRP_BENCH_GRID_CACHE").filter(|d| !d.is_empty())?);
            let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
            let source = source_fingerprint(&root).unwrap_or_else(|why| {
                panic!("SRP_BENCH_GRID_CACHE: the grid builder's source at {} is not readable ({why}): no key for its files", root.display())
            });
            make_shared_dir(&dir).unwrap_or_else(|why| panic!("SRP_BENCH_GRID_CACHE={}: {why}", dir.display()));
            eprintln!("  grid cache: {} (builder source {source:016x})", dir.display());
            Some(Cache { dir, source })
        })
        .as_ref()
}

/// `dir`, made when missing, writable by every user (with the sticky bit: each user's files are only that user's to replace) so
/// the sessions' benches share one cache.
fn make_shared_dir(dir: &Path) -> std::io::Result<()> {
    if dir.is_dir() {
        return Ok(());
    }
    std::fs::create_dir_all(dir)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o1777))?;
    }
    Ok(())
}

/// The digest of the grid builder's source under the workspace `root` (see the module's documentation): each file's path from
/// `root` and its bytes, in the order of the paths.
fn source_fingerprint(root: &Path) -> std::io::Result<u64> {
    let mut files = vec![
        PathBuf::from("Cargo.toml"),
        PathBuf::from("Cargo.lock"),
        PathBuf::from("flake.nix"),
        PathBuf::from("flake.lock"),
    ];
    let mut dirs = vec![PathBuf::from("crates/mapdata")];
    while let Some(dir) = dirs.pop() {
        for entry in std::fs::read_dir(root.join(&dir))? {
            let entry = entry?;
            let path = dir.join(entry.file_name());
            if entry.file_type()?.is_dir() {
                dirs.push(path);
            } else {
                files.push(path);
            }
        }
    }
    files.sort();
    let mut digest = Digest::new();
    for file in &files {
        digest.bytes(file.to_string_lossy().replace('\\', "/").as_bytes());
        digest.bytes(&std::fs::read(root.join(file))?);
    }
    Ok(digest.finish())
}

/// How a part of the world came.
#[derive(Clone, Copy, Debug)]
enum Made {
    Read,
    Built,
}

/// [`World::build_with_extras`] of `map`, its grid and sight tree read from the run's cache when it has them (and written there
/// when built); with no cache, `World::build_with_extras` itself.
pub(super) fn world(
    map: MapData,
    cell: f32,
    body: Body,
    links: OffMeshLinks,
    extras: GridExtras,
) -> World {
    let Some(cache) = cache() else {
        return World::build_with_extras(map, cell, body, links, extras);
    };
    // As `World::build_with_extras`, each part by its key.
    let started = Instant::now();
    let spawns: Vec<mapdata::Vec3> = map.spawns.iter().map(|s| s.position).collect();
    let walker = map.walker_triangles();
    let area = map.play_area(mapdata::AREA_MARGIN);
    let seeds = map.grid_seeds();
    let grid_key = {
        let mut digest = Digest::new();
        digest.u64(mapdata::grid_key_with_links(
            &walker, area, cell, body, &seeds, links,
        ));
        digest.len(spawns.len());
        for spawn in &spawns {
            [spawn.x, spawn.y, spawn.z]
                .into_iter()
                .for_each(|v| digest.f32(v));
        }
        digest.finish()
    };
    let (grid, grid_made) = cache.read_or_build(
        format!("bench-grid-{:016x}-{grid_key:016x}.bin", cache.source),
        |path| WalkGrid::read(path, grid_key, body),
        || WalkGrid::build_kept_with_links(&walker, area, cell, body, &seeds, &spawns, links).0,
        |grid, path| grid.write(path, grid_key),
    );
    let grid_s = started.elapsed().as_secs_f32();
    let grid = extras.apply(grid, &walker);
    drop(walker);
    let extras_s = started.elapsed().as_secs_f32() - grid_s;
    let bullet = map.bullet_mesh();
    let sight_key = bullet.key();
    let (sight, sight_made) = cache.read_or_build(
        format!("bench-sight-{:016x}-{sight_key:016x}.bin", cache.source),
        |path| RayTree::read(path, sight_key),
        || bullet.build_tree(),
        |sight, path| sight.write(path, sight_key),
    );
    drop(bullet);
    let sight_s = started.elapsed().as_secs_f32() - grid_s - extras_s;
    eprintln!(
        "  grid cache: grid {grid_made:?} {grid_s:.2} s (key {grid_key:016x}), its tables {extras_s:.2} s, sight tree {sight_made:?} {sight_s:.2} s (key {sight_key:016x})"
    );
    World::assemble(map, grid.into(), sight.into())
}

impl Cache {
    /// The part in `file_name` if it reads back, else built by `build` and written there (to a name of its own, renamed in). On a
    /// miss the run holds the key's lock file (`<file_name>.lock`, the system's lock, let go when the run ends however it ends)
    /// and looks again: the jobs of one map started together build it once, the others read what it wrote.
    fn read_or_build<T>(
        &self,
        file_name: String,
        read: impl Fn(&Path) -> Result<T, mapdata::Error>,
        build: impl FnOnce() -> T,
        write: impl FnOnce(&T, &Path) -> Result<(), mapdata::Error>,
    ) -> (T, Made) {
        let path = self.dir.join(&file_name);
        let read_back = || match path.is_file().then(|| read(&path)) {
            Some(Ok(value)) => Some(value),
            Some(Err(why)) => {
                eprintln!("  grid cache: {file_name} refused ({why})");
                None
            }
            None => None,
        };
        if let Some(value) = read_back() {
            return (value, Made::Read);
        }
        let lock_path = self.dir.join(format!("{file_name}.lock"));
        // Another user's lock file opens for reading only (the folder's sticky bit); the lock works on either.
        let lock = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(false)
            .open(&lock_path)
            .or_else(|_| std::fs::File::open(&lock_path))
            .and_then(|file| file.lock().map(|()| file));
        match &lock {
            Ok(_) => {
                if let Some(value) = read_back() {
                    return (value, Made::Read);
                }
            }
            Err(why) => eprintln!(
                "  grid cache: {} not locked ({why}); built without the lock",
                lock_path.display()
            ),
        }
        let value = build();
        let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos());
        let own = self.dir.join(format!(".part-{}-{nanos}-{file_name}", std::process::id()));
        let written = write(&value, &own)
            .map_err(|e| e.to_string())
            .and_then(|()| std::fs::rename(&own, &path).map_err(|e| format!("{}: {e}", path.display())));
        if let Err(why) = written {
            let _ = std::fs::remove_file(&own);
            eprintln!("  grid cache: {file_name} not written ({why}); this run built it");
        }
        (value, Made::Built)
    }
}
