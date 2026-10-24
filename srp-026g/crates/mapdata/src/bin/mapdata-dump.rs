//! Writes one map data file per project from the game's `resources.db`:
//!
//! ```text
//! mapdata-dump --db <resources.db> --out <dir>                  # every playable project
//! mapdata-dump --db <resources.db> --out <dir> --map 117 --map 87
//! mapdata-dump --db <resources.db> --out <dir> --project level_01/day_4 --stats 0.5
//! mapdata-dump --db <resources.db> --patch <resources.db.patch_14> --patch <...> --out <dir> \
//!     --project level_09/day_2
//! ```
//!
//! `--patch` lays a resources.db patch over the archive as the game mounts
//! it (repeatable, in mount order: ascending patch number); a map that only
//! a patch has (London, level_09) is dumped with `--project`, since `--map`
//! knows the stock maps only.
//!
//! The files are the game's content: keep them out of the repository.
//! `--stats <cell>` also builds the walk grid and the sight tree of each
//! project and prints counts and timings.

use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Instant;

use clap::Parser;
use mapdata::{Body, MapData, Materials, Resources, Vec3, World, format};
use survarium::v026g::maps;

#[derive(Parser)]
#[command(about = "Dump 0.26g map data (spawns, objectives, collision) per project")]
struct Cli {
    /// The game's resources.db.
    #[arg(long, env = mapdata::resources::PATH_VARIABLE)]
    db: PathBuf,
    /// A resources.db patch laid over it; repeatable, in mount order.
    #[arg(long = "patch")]
    patches: Vec<PathBuf>,
    /// Where the `.mapdata` files go.
    #[arg(long)]
    out: PathBuf,
    /// A map id (`maps_dict`, e.g. 117); repeatable.
    #[arg(long = "map")]
    maps: Vec<u8>,
    /// A project (`level_01/day_4`); repeatable.
    #[arg(long = "project")]
    projects: Vec<String>,
    /// Also build the walk grid with this cell size and print timings.
    #[arg(long)]
    stats: Option<f32>,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match run(&cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run(cli: &Cli) -> Result<(), Box<dyn std::error::Error>> {
    let mut projects: Vec<String> = cli.projects.clone();
    for &id in &cli.maps {
        let map = maps::map_mode(id)
            .filter(|map| map.is_playable())
            .ok_or_else(|| format!("map {id} is not a playable map"))?;
        projects.push(map.project.to_owned());
    }
    if projects.is_empty() {
        projects = maps::playable().map(|map| map.project.to_owned()).collect();
    }
    projects.sort();
    projects.dedup();

    let started = Instant::now();
    let mut layers = vec![cli.db.clone()];
    layers.extend(cli.patches.iter().cloned());
    let mut resources = Resources::open_layers(&layers)?;
    if resources.unresolved_links > 0 {
        eprintln!(
            "warning: {} hard links in the index point at no file",
            resources.unresolved_links
        );
    }
    let materials = Materials::load(&mut resources)?;
    eprintln!(
        "opened {} in {:.2} s",
        cli.db.display(),
        started.elapsed().as_secs_f32()
    );
    std::fs::create_dir_all(&cli.out)?;

    for project in &projects {
        let t = Instant::now();
        let map = MapData::load(&mut resources, &materials, project)?;
        let load = t.elapsed().as_secs_f32();
        let path = cli.out.join(format::file_name(project));
        map.write(&path)?;
        let size = std::fs::metadata(&path)?.len();
        let t = Instant::now();
        let map = MapData::read(&path)?;
        let read = t.elapsed().as_secs_f32();
        println!(
            "{project}: {} spawns, {} containers, {} battery spawners, {} stations, {} zones, \
             {} libraries, {} placements; {:.1} MB; dumped in {load:.2} s, read in {read:.3} s",
            map.spawns.len(),
            map.containers.len(),
            map.battery_spawners.len(),
            map.stations.len(),
            map.zones.len(),
            map.libraries.len(),
            map.placements.len(),
            size as f64 / 1e6,
        );
        if let Some(cell) = cli.stats {
            stats(map, cell);
        }
    }
    Ok(())
}

fn stats(map: MapData, cell: f32) {
    let t = Instant::now();
    let walker = map.walker_triangles();
    let bullet = map.bullet_triangles();
    let place = t.elapsed().as_secs_f32();
    let area = map.play_area(mapdata::AREA_MARGIN);
    let t = Instant::now();
    let spawns: Vec<mapdata::Vec3> = map.spawns.iter().map(|s| s.position).collect();
    let (grid, built) = mapdata::WalkGrid::build_kept_with_stats(
        &walker,
        area,
        cell,
        Body::default(),
        &map.grid_seeds(),
        &spawns,
    );
    let grid_time = t.elapsed().as_secs_f32();
    let t = Instant::now();
    let sight = mapdata::RayTree::build(&bullet);
    let sight_time = t.elapsed().as_secs_f32();
    println!(
        "  placed {} walker + {} bullet triangles in {place:.2} s; grid {}x{} of {cell} m: \
         {} floors, {} after erosion, {} kept, {:.1} MB, {grid_time:.2} s; sight tree {:.1} MB, {sight_time:.2} s",
        walker.len(),
        bullet.len(),
        grid.dimensions().0,
        grid.dimensions().1,
        built.floors,
        built.floors_after_erosion,
        built.floors_kept,
        grid.memory_bytes() as f64 / 1e6,
        sight.memory_bytes() as f64 / 1e6,
    );
    let world = World::assemble(map, grid.into(), sight.into());
    let spawns: Vec<(i32, Option<u32>)> = world
        .map
        .spawns
        .iter()
        .map(|s| (s.team, world.grid.locate(s.position, 1.0)))
        .collect();
    let located: Vec<u32> = spawns.iter().filter_map(|s| s.1).collect();
    let t = Instant::now();
    let reach: Vec<usize> = [0, 1]
        .iter()
        .map(|&team| {
            let own: Vec<u32> = spawns
                .iter()
                .filter(|s| s.0 == team)
                .filter_map(|s| s.1)
                .collect();
            let from = world.grid.distances_from(&own);
            located.iter().filter(|&&n| from.reaches(n)).count()
        })
        .collect();
    let flood = t.elapsed().as_secs_f32() / 2.0;
    println!(
        "  spawns located {}/{}; team 0 reaches {}/{} of them, team 1 {}/{}; flood {:.1} ms",
        located.len(),
        spawns.len(),
        reach[0],
        located.len(),
        reach[1],
        located.len(),
        flood * 1e3,
    );
    // A* between the first few spawns of each team.
    let team = |t: i32| -> Vec<u32> {
        spawns
            .iter()
            .filter(|s| s.0 == t)
            .filter_map(|s| s.1)
            .take(5)
            .collect()
    };
    let (a, b) = (team(0), team(1));
    let t = Instant::now();
    let (mut found, mut ratio, mut runs) = (0, 0.0, 0);
    for &x in &a {
        for &y in &b {
            runs += 1;
            if let Some(path) = world.grid.path(x, y) {
                found += 1;
                let straight = world.grid.position(x).distance_xz(world.grid.position(y));
                ratio += path.cost / straight.max(1.0);
            }
        }
    }
    if runs > 0 {
        println!(
            "  A* team 0 -> team 1 spawns: {found}/{runs} found, path / straight {:.2}, {:.1} ms a search",
            ratio / found.max(1) as f32,
            t.elapsed().as_secs_f32() * 1e3 / runs as f32,
        );
    }
    // Rays between random floors, eye to chest.
    let n = world.grid.len() as u64;
    if n > 0 {
        let mut seed = 0x9e37_79b9_7f4a_7c15u64;
        let mut next = move || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            seed
        };
        let mut rays = Vec::new();
        while rays.len() < 100_000 {
            let p = world.grid.position((next() % n) as u32) + Vec3::new(0.0, 1.6, 0.0);
            let q = world.grid.position((next() % n) as u32) + Vec3::new(0.0, 1.2, 0.0);
            let d = (q - p).length();
            if (5.0..=150.0).contains(&d) {
                rays.push((p, q));
            }
        }
        let t = Instant::now();
        let clear = rays
            .iter()
            .filter(|(p, q)| world.sight.visible(*p, *q))
            .count();
        println!(
            "  {} rays 5-150 m: {:.2} us a ray, {:.0} % clear",
            rays.len(),
            t.elapsed().as_secs_f64() * 1e6 / rays.len() as f64,
            100.0 * clear as f64 / rays.len() as f64,
        );
    }
}
