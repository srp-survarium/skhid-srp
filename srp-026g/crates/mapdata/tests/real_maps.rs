//! Checks on the game's own maps. They run only when `SRP_RESOURCES_DB`
//! names the 0.26g `resources.db` (and skip with a note otherwise):
//!
//! ```text
//! SRP_RESOURCES_DB=<path to resources.db> cargo test -p mapdata --release -- --nocapture
//! ```

use std::time::Instant;

use mapdata::{Body, MapData, Materials, NodeId, RayTree, Resources, Vec3, WalkGrid, World};
use survarium::v026g::maps;

fn load(map_id: u8) -> Option<MapData> {
    let Some(path) = std::env::var_os(mapdata::resources::PATH_VARIABLE) else {
        eprintln!("skipping: {} is not set", mapdata::resources::PATH_VARIABLE);
        return None;
    };
    let mut resources = match Resources::open(path.as_ref()) {
        Ok(resources) => resources,
        Err(error) => {
            eprintln!("skipping: game data not available ({error})");
            return None;
        }
    };
    assert_eq!(resources.unresolved_links, 0);
    let materials = Materials::load(&mut resources).unwrap();
    let project = maps::map_mode(map_id).unwrap().project;
    let t = Instant::now();
    let map = MapData::load(&mut resources, &materials, project).unwrap();
    eprintln!("{project}: loaded in {:.2} s", t.elapsed().as_secs_f32());
    Some(map)
}

/// Height of each spawn above the walker mesh under it.
fn spawn_heights(map: &MapData) -> Vec<f32> {
    let walker = RayTree::build(&map.walker_triangles());
    map.spawns
        .iter()
        .map(|s| {
            let from = s.position + Vec3::new(0.0, 1.0, 0.0);
            walker
                .ray_cast(from, Vec3::new(0.0, -1.0, 0.0), 4.0)
                .map_or(f32::INFINITY, |t| t - 1.0)
        })
        .collect()
}

/// The spawns' points: the grid's pockets are theirs.
fn spawns_of(map: &MapData) -> Vec<Vec3> {
    map.spawns.iter().map(|s| s.position).collect()
}

fn grid(map: &MapData, cell: f32, body: Body) -> WalkGrid {
    let t = Instant::now();
    let grid = WalkGrid::build_kept(
        &map.walker_triangles(),
        map.play_area(mapdata::AREA_MARGIN),
        cell,
        body,
        &map.grid_seeds(),
        &spawns_of(map),
    );
    eprintln!(
        "grid of {cell} m: {} floors in {:.2} s",
        grid.len(),
        t.elapsed().as_secs_f32()
    );
    grid
}

/// The spawns' floors, by team.
fn spawn_nodes(map: &MapData, grid: &WalkGrid, team: i32) -> Vec<NodeId> {
    map.spawns
        .iter()
        .filter(|s| s.team == team)
        .map(|s| grid.locate(s.position, 1.0).expect("spawn on the grid"))
        .collect()
}

#[test]
fn level_01_day_4_spawns_stand_on_the_walker_mesh_and_connect() {
    let Some(map) = load(117) else {
        return;
    };
    assert_eq!(map.project, "level_01/day_4");
    assert_eq!(map.spawns.len(), 43);
    assert_eq!(map.spawns.iter().filter(|s| s.team == 0).count(), 26);
    assert_eq!(map.placements.len(), 1015);
    // The placement rule (suffix scale, no object scale, Rx·Ry·Rz):
    // every spawn is on the walking surface.
    let heights = spawn_heights(&map);
    let (low, high) = heights
        .iter()
        .fold((f32::MAX, f32::MIN), |(l, h), &x| (l.min(x), h.max(x)));
    eprintln!("spawn height above the walker mesh: {low:+.3} .. {high:+.3} m");
    assert!(heights.iter().all(|h| h.abs() <= 0.15), "{heights:?}");

    // The file keeps all of it.
    assert_eq!(MapData::from_bytes(&map.to_bytes()).unwrap(), map);

    // 0.5 m cells, the prototype's measures: without jumps 34 of the 43
    // spawns are in one piece. (With jumps each team got to every spawn
    // until a jump's foot became a wall, 2026-10-23: at 0.5 m that wall
    // takes a whole cell and some ways by a ledge shut; the grid of use,
    // 0.25 m, is checked below.)
    let coarse = grid(&map, 0.5, Body::default());
    let all: Vec<NodeId> = [0, 1]
        .iter()
        .flat_map(|&team| spawn_nodes(&map, &coarse, team))
        .collect();
    for team in [0, 1] {
        let reach = coarse.distances_from(&spawn_nodes(&map, &coarse, team));
        let got = all.iter().filter(|&&n| reach.reaches(n)).count();
        eprintln!("0.5 m, team {team} gets to {got}/{} spawns", all.len());
    }
    let no_jumps = Body {
        max_jump: 0.0,
        ..Body::default()
    };
    let flat = grid(&map, 0.5, no_jumps);
    let nodes: Vec<NodeId> = map
        .spawns
        .iter()
        .map(|s| flat.locate(s.position, 1.0).unwrap())
        .collect();
    let best = nodes
        .iter()
        .map(|&n| {
            let reach = flat.distances_from(&[n]);
            nodes.iter().filter(|&&m| reach.reaches(m)).count()
        })
        .max()
        .unwrap();
    eprintln!("0.5 m, no jumps: at most {best}/43 spawns in one piece");
    assert_eq!(best, 34);

    // 0.25 m cells: every spawn gets to every other one (a few 0.5 m
    // pockets behind 1 m doors open).
    let t = Instant::now();
    let world = World::build(map, 0.25, Body::default());
    eprintln!("world of 0.25 m in {:.2} s", t.elapsed().as_secs_f32());
    let nodes: Vec<NodeId> = world
        .map
        .spawns
        .iter()
        .map(|s| {
            world
                .grid
                .locate(s.position, 1.0)
                .expect("spawn on the grid")
        })
        .collect();
    let t = Instant::now();
    for (i, &from) in nodes.iter().enumerate() {
        let reach = world.grid.distances_from(&[from]);
        let missed: Vec<usize> = (0..nodes.len())
            .filter(|&j| !reach.reaches(nodes[j]))
            .collect();
        assert!(missed.is_empty(), "spawn {i} reaches none of {missed:?}");
    }
    eprintln!(
        "{} floods in {:.2} s",
        nodes.len(),
        t.elapsed().as_secs_f32()
    );

    // A path between the teams' first spawns, and sight along it.
    let first = |team| {
        world
            .map
            .spawns
            .iter()
            .position(|s| s.team == team)
            .unwrap()
    };
    let (a, b) = (nodes[first(0)], nodes[first(1)]);
    let t = Instant::now();
    let path = world.grid.path(a, b).expect("a path between the teams");
    let straight = world.grid.position(a).distance_xz(world.grid.position(b));
    eprintln!(
        "A*: {:.0} m for {straight:.0} m straight, {:.1} ms",
        path.cost,
        t.elapsed().as_secs_f64() * 1e3
    );
    assert!(path.cost >= straight && path.cost < 2.0 * straight);
    let to_b = world.grid.distances_to(&[b]);
    assert!((to_b.get(a) - path.cost).abs() < 0.01 * path.cost);
    let eye = Vec3::new(0.0, 1.6, 0.0);
    let clear = path
        .nodes
        .windows(2)
        .filter(|w| {
            let (p, q) = (world.grid.position(w[0]), world.grid.position(w[1]));
            world.sight.visible(p + eye, q + eye)
        })
        .count();
    // Neighbouring floors of a path see each other (almost always: a
    // bullet-only prop may stand across a cell's corner).
    assert!(clear as f32 >= 0.95 * (path.nodes.len() - 1) as f32);
}

#[test]
fn level_05_evening_2_spawn_areas_are_left_one_way() {
    let Some(map) = load(87) else {
        return;
    };
    assert_eq!(map.spawns.len(), 29);
    assert_eq!(map.containers.len(), 2);
    assert_eq!(map.battery_spawners.len(), 50);
    let heights = spawn_heights(&map);
    assert!(heights.iter().all(|h| h.abs() <= 0.15), "{heights:?}");
    let world = World::build(map, 0.25, Body::default());
    for team in [0, 1] {
        let own = spawn_nodes(&world.map, &world.grid, team);
        let reach = world.grid.distances_from(&own);
        let enemy = spawn_nodes(&world.map, &world.grid, 1 - team)
            .into_iter()
            .filter(|&n| reach.reaches(n))
            .count();
        let containers: Vec<String> = world
            .map
            .containers
            .iter()
            .map(|c| {
                let node = world.grid.locate(c.position, 2.0);
                let use_at = c
                    .use_shapes
                    .first()
                    .and_then(|s| world.grid.locate(s.center, 2.0));
                format!(
                    "team {}'s at {:?}: located {} reached {}, use box located {} reached {}",
                    c.team,
                    c.position,
                    node.is_some(),
                    node.is_some_and(|n| reach.reaches(n)),
                    use_at.is_some(),
                    use_at.is_some_and(|n| reach.reaches(n)),
                )
            })
            .collect();
        let batteries = world
            .map
            .battery_spawners
            .iter()
            .filter_map(|s| world.grid.locate(s.item + Vec3::new(0.0, 0.3, 0.0), 1.0))
            .filter(|&n| reach.reaches(n))
            .count();
        eprintln!(
            "team {team}: reaches {:.0} m², {enemy} enemy spawns, containers {containers:?}, \
             {batteries}/50 batteries",
            reach.reached() as f32 * 0.0625,
        );
        // Out of the spawn area one way only (the owner: plausible), and
        // every battery within reach.
        assert_eq!(enemy, 0);
        assert_eq!(batteries, 50);
    }
}

/// Whether a level's weather projects (day, evening, rain) share their
/// walk and bullet geometry: one built world could serve all three.
#[test]
#[ignore = "needs SRP_MAPDATA_DIR"]
fn weather_projects_share_geometry() {
    let Some(dir) = std::env::var_os("SRP_MAPDATA_DIR").map(std::path::PathBuf::from) else {
        return;
    };
    for level in 1..=8 {
        for mode in 2..=4 {
            let read = |weather: &str| {
                let project = format!("level_{level:02}/{weather}_{mode}");
                MapData::read(&dir.join(mapdata::format::file_name(&project))).ok()
            };
            let Some(day) = read("day") else { continue };
            for weather in ["evening", "rain", "prerain"] {
                let Some(other) = read(weather) else { continue };
                let same_libraries = day.libraries.len() == other.libraries.len()
                    && day.libraries.iter().zip(&other.libraries).all(|(a, b)| {
                        a.vertices == b.vertices
                            && a.faces == b.faces
                    });
                let same_placements = day.placements == other.placements;
                let same_box = day.levelmap_min == other.levelmap_min
                    && day.levelmap_max == other.levelmap_max;
                eprintln!(
                    "level_{level:02} mode {mode} day vs {weather}: libraries {same_libraries}, placements {same_placements}, box {same_box}, spawns {}, zones {}",
                    day.spawns == other.spawns,
                    day.zones.len() == other.zones.len()
                );
            }
        }
    }
}

/// Every world in `SRP_MAPDATA_DIR` builds the same twice, and its digests
/// (`project,floors,grid,sight`) go to `SRP_DIGESTS_OUT` when set: the
/// baseline that a world read from a file, shared or stored more compactly
/// must match (`docs/notes/player_bots_mapdata.md` §6).
#[test]
#[ignore = "needs SRP_MAPDATA_DIR"]
fn every_world_builds_the_same_twice() {
    let Some(dir) = std::env::var_os("SRP_MAPDATA_DIR").map(std::path::PathBuf::from) else {
        return;
    };
    let mut files: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "mapdata"))
        .collect();
    files.sort();
    assert!(!files.is_empty(), "no map files in {}", dir.display());
    let mut lines = vec!["project,floors,grid,sight".to_owned()];
    for path in &files {
        let build = || {
            let map = MapData::read(path).unwrap();
            World::build(map, mapdata::CELL, Body::default())
        };
        let t = Instant::now();
        let world = build();
        let seconds = t.elapsed().as_secs_f32();
        let (grid, sight) = (world.grid.digest(), world.sight.digest());
        let again = build();
        assert_eq!(again.grid.digest(), grid, "{}", world.map.project);
        assert_eq!(again.sight.digest(), sight, "{}", world.map.project);
        eprintln!(
            "{}: {} floors, built in {seconds:.2} s, grid {grid:016x}, sight {sight:016x}",
            world.map.project,
            world.grid.len()
        );
        lines.push(format!(
            "{},{},{grid:016x},{sight:016x}",
            world.map.project,
            world.grid.len()
        ));
    }
    if let Some(out) = std::env::var_os("SRP_DIGESTS_OUT") {
        std::fs::write(out, lines.join("\n") + "\n").unwrap();
    }
}

/// Every world's grid and tree written to `SRP_BUILT_DIR` (by key, once
/// per geometry) read back the same, and how long a read takes against a
/// build.
#[test]
#[ignore = "needs SRP_MAPDATA_DIR and SRP_BUILT_DIR"]
fn every_world_reads_back_from_its_built_files() {
    let (Some(dir), Some(out)) = (
        std::env::var_os("SRP_MAPDATA_DIR").map(std::path::PathBuf::from),
        std::env::var_os("SRP_BUILT_DIR").map(std::path::PathBuf::from),
    ) else {
        return;
    };
    std::fs::create_dir_all(&out).unwrap();
    let mut files: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "mapdata"))
        .collect();
    files.sort();
    let (mut built_s, mut read_s, mut bytes) = (0.0, 0.0, 0u64);
    for path in &files {
        let map = MapData::read(path).unwrap();
        let walker = map.walker_triangles();
        let area = map.play_area(mapdata::AREA_MARGIN);
        let body = Body::default();
        let seeds = map.grid_seeds();
        let grid_key = mapdata::grid_key(&walker, area, mapdata::CELL, body, &seeds);
        let bullet = map.bullet_mesh();
        let sight_key = bullet.key();
        let grid_path = out.join(mapdata::built::grid_file_name(grid_key));
        let sight_path = out.join(mapdata::built::sight_file_name(sight_key));
        let t = Instant::now();
        let grid = WalkGrid::build_kept(&walker, area, mapdata::CELL, body, &seeds, &spawns_of(&map));
        let sight = bullet.build_tree();
        let build = t.elapsed().as_secs_f32();
        if !grid_path.is_file() {
            grid.write(&grid_path, grid_key).unwrap();
        }
        if !sight_path.is_file() {
            sight.write(&sight_path, sight_key).unwrap();
        }
        let t = Instant::now();
        let read_grid = WalkGrid::read(&grid_path, grid_key, body).unwrap();
        let read_sight = RayTree::read(&sight_path, sight_key).unwrap();
        let read = t.elapsed().as_secs_f32();
        assert_eq!(read_grid.digest(), grid.digest(), "{}", map.project);
        assert_eq!(read_sight.digest(), sight.digest(), "{}", map.project);
        built_s += build;
        read_s += read;
        bytes += std::fs::metadata(&grid_path).unwrap().len()
            + std::fs::metadata(&sight_path).unwrap().len();
        eprintln!(
            "{}: built in {build:.2} s, read in {read:.2} s (grid {grid_key:016x}, sight {sight_key:016x})",
            map.project
        );
    }
    let on_disk: u64 = std::fs::read_dir(&out)
        .unwrap()
        .map(|entry| entry.unwrap().metadata().unwrap().len())
        .sum();
    eprintln!(
        "{} projects: built in {built_s:.1} s, read in {read_s:.1} s; files {} MB on disk",
        files.len(),
        on_disk >> 20
    );
    let _ = bytes;
}

/// Each world's kept grid is its whole grid less floors no body gets to:
/// every kept floor is a floor of the whole at the same place, with the
/// same headroom and links; every floor the seeds reach in the whole is
/// kept; and how many go and what memory that saves, per map.
#[test]
#[ignore = "needs SRP_MAPDATA_DIR"]
fn kept_grids_are_the_whole_grids_less_floors_no_body_gets_to() {
    let Some(dir) = std::env::var_os("SRP_MAPDATA_DIR").map(std::path::PathBuf::from) else {
        return;
    };
    let mut files: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "mapdata"))
        .collect();
    files.sort();
    let key = |p: Vec3| (p.x.to_bits(), p.y.to_bits(), p.z.to_bits());
    let (mut whole_bytes, mut kept_bytes) = (0usize, 0usize);
    for path in &files {
        let map = MapData::read(path).unwrap();
        let walker = map.walker_triangles();
        let area = map.play_area(mapdata::AREA_MARGIN);
        let body = Body::default();
        let seeds = map.grid_seeds();
        let whole = WalkGrid::build(&walker, area, mapdata::CELL, body);
        let kept = WalkGrid::build_kept(&walker, area, mapdata::CELL, body, &seeds, &[]);
        let of: std::collections::HashMap<_, NodeId> = (0..whole.len() as NodeId)
            .map(|f| (key(whole.position(f)), f))
            .collect();
        let mut present = vec![false; whole.len()];
        for k in 0..kept.len() as NodeId {
            let f = of[&key(kept.position(k))];
            present[f as usize] = true;
            assert_eq!(kept.headroom(k), whole.headroom(f));
            let links = |g: &WalkGrid, a: NodeId| -> Vec<_> {
                g.links(a)
                    .map(|(b, kind)| (key(g.position(b)), kind))
                    .collect()
            };
            assert_eq!(links(&kept, k), links(&whole, f), "{}", map.project);
        }
        // What the seeds reach in the whole is kept.
        let starts: Vec<NodeId> = seeds.iter().filter_map(|&s| whole.locate(s, 3.0)).collect();
        let mut seen = vec![false; whole.len()];
        let mut queue = starts.clone();
        for &s in &starts {
            seen[s as usize] = true;
        }
        while let Some(a) = queue.pop() {
            assert!(
                present[a as usize],
                "{}: a reached floor dropped",
                map.project
            );
            for (b, _) in whole.links(a) {
                if !seen[b as usize] {
                    seen[b as usize] = true;
                    queue.push(b);
                }
            }
        }
        // A seed finds the same floor in both.
        for &s in &seeds {
            assert_eq!(
                whole.locate(s, 3.0).map(|f| key(whole.position(f))),
                kept.locate(s, 3.0).map(|k| key(kept.position(k))),
                "{}",
                map.project
            );
        }
        whole_bytes += whole.memory_bytes();
        kept_bytes += kept.memory_bytes();
        eprintln!(
            "{}: {} floors, {} kept ({:.0} %), {:.1} MB to {:.1} MB",
            map.project,
            whole.len(),
            kept.len(),
            100.0 * kept.len() as f64 / whole.len().max(1) as f64,
            whole.memory_bytes() as f64 / 1e6,
            kept.memory_bytes() as f64 / 1e6
        );
    }
    eprintln!(
        "all {} projects: {:.0} MB to {:.0} MB",
        files.len(),
        whole_bytes as f64 / 1e6,
        kept_bytes as f64 / 1e6
    );
}
