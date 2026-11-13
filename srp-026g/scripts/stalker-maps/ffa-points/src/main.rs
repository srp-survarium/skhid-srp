//! Author FFA points on the three STALKER maps. Explicit TSV output is reviewed
//! and committed, then ../make_ffa_edits.py creates the source edit list.
//! Candidates come from reachable standing floors, pass the same collision
//! checks as imported points, stay clear of harmful zones, and have a six-metre
//! walkable facing. Greedy farthest-point selection distributes 24 points.
//! Training Camp then adds every point its raised islands
//! (training-islands.tsv) take at the same spacing from all of them.
mod collision;
mod arena;
use collision::{Map, Verdict};
use mapdata::{Materials, Resources, Vec3};
use std::{collections::BTreeSet, fs::File, io::Write, path::PathBuf, process::ExitCode};

const COUNT: usize = 24;
const MIN_SPACING: f32 = 12.0;
const HAZARD_MARGIN: f32 = 2.0;
const FACING_REACH: f32 = 6.0;
const TRAINING: &str = "v010b_cs_training_camp/day_4";
/// The islands are small: their floors are sampled finer than the 3 m lattice.
const ISLAND_LATTICE: f32 = 0.75;

/// Bounds of the reviewed port's minimaps (stalker-maps@6). The imported
/// projects have a dummy 2 m levelmap; mapdata's native-map fallback truncates
/// Military Warehouses. Training Camp has open scenery beyond its play area.
fn area(project: &str) -> Result<([f32; 4], Option<f32>), String> {
    match project {
        "v010b_cs_mp_bath/day_4" => Ok(([-48.13, -22.87, 34.12, 53.13], None)),
        "v010b_cs_mp_military_1/day_4" => Ok(([-127.47, -71.12, 115.78, 95.38], None)),
        "v010b_cs_training_camp/day_4" => Ok(([-47.19, -83.47, 82.81, 91.53], None)),
        _ => Err(format!("no reviewed play area for {project}")),
    }
}

fn in_play_area(map: &Map, p: Vec3) -> bool {
    let (bounds, radius) = area(&map.data.project).expect("checked project");
    p.x >= bounds[0]
        && p.z >= bounds[1]
        && p.x <= bounds[2]
        && p.z <= bounds[3]
        && (map.data.project != TRAINING || arena::contains(p.x, p.z, 2.0))
        && radius.is_none_or(|r| {
            map.data
                .spawns
                .iter()
                .any(|s| s.position.distance_xz(p) <= r)
        })
}

#[derive(Clone, Copy)]
struct Candidate {
    position: Vec3,
    yaw: f32,
}

fn harmful(map: &Map, p: Vec3) -> bool {
    map.data
        .zones
        .iter()
        .filter(|z| z.damage.as_ref().is_some_and(|d| d.harmful()))
        .flat_map(|z| &z.shapes)
        .any(|s| {
            [0.1, 0.9, 1.7]
                .iter()
                .any(|h| s.distance(p + Vec3::new(0.0, *h, 0.0)) < HAZARD_MARGIN)
        })
}

fn facing(map: &Map, node: u32, center: Vec3) -> Option<f32> {
    let p = map.grid.position(node);
    let eye = p + Vec3::new(0.0, 1.6, 0.0);
    let toward = center - p;
    (0..32)
        .filter_map(|i| {
            let yaw = i as f32 * std::f32::consts::TAU / 32.0;
            let dir = mapdata::math::forward(Vec3::new(0.0, yaw, 0.0));
            let destination = p + dir * FACING_REACH;
            let end = map.grid.locate(destination, 0.4)?;
            let at = map.grid.position(end);
            if !map.grid.can_stand(end)
                || (at.y - p.y).abs() > 0.6
                || harmful(map, at)
                || !in_play_area(map, at)
                || !map
                    .grid
                    .walks_straight(node, end, FACING_REACH + 0.5, |n| map.grid.can_stand(n))
            {
                return None;
            }
            let sight = map.rays.ray_cast(eye, dir, 20.0).unwrap_or(20.0);
            // Prefer room to see ahead, then look into the play area on a tie.
            // Project rotations follow Vostok's create_rotation_y: +z turns
            // toward -x for positive yaw. Store the heading of the direction
            // we actually checked, not the sampling angle's opposite yaw.
            Some((
                mapdata::math::heading(dir),
                sight,
                dir.x * toward.x + dir.z * toward.z,
            ))
        })
        .max_by(|a, b| a.1.total_cmp(&b.1).then(a.2.total_cmp(&b.2)))
        .map(|(yaw, _, _)| yaw)
}

fn choose(points: &[Candidate], count: usize, spacing: f32) -> Result<Vec<usize>, String> {
    if points.is_empty() {
        return Err("no safe standing candidates".into());
    }
    let lo = points
        .iter()
        .fold(points[0].position, |a, p| a.min(p.position));
    let hi = points
        .iter()
        .fold(points[0].position, |a, p| a.max(p.position));
    let center = (lo + hi) * 0.5;
    let first = (0..points.len())
        .min_by(|&a, &b| {
            points[a]
                .position
                .distance_xz(center)
                .total_cmp(&points[b].position.distance_xz(center))
        })
        .unwrap();
    let mut chosen = vec![first];
    let mut distance = vec![f32::INFINITY; points.len()];
    while chosen.len() < count {
        let last = points[*chosen.last().unwrap()].position;
        for (i, p) in points.iter().enumerate() {
            distance[i] = distance[i].min(p.position.distance_xz(last));
        }
        let next = (0..points.len())
            .max_by(|&a, &b| distance[a].total_cmp(&distance[b]).then(b.cmp(&a)))
            .unwrap();
        if distance[next] < spacing {
            return Err(format!(
                "only {} points fit at {spacing} m spacing",
                chosen.len()
            ));
        }
        chosen.push(next);
    }
    Ok(chosen)
}

/// More points after `chosen`: the candidate farthest from every point so far,
/// as long as one keeps `spacing` from them all.
fn extend(points: &[Candidate], chosen: &[Vec3], spacing: f32) -> Vec<usize> {
    let mut distance: Vec<f32> = points
        .iter()
        .map(|p| {
            chosen
                .iter()
                .map(|at| p.position.distance_xz(*at))
                .fold(f32::INFINITY, f32::min)
        })
        .collect();
    let mut added = Vec::new();
    while let Some(next) = (0..points.len())
        .max_by(|&a, &b| distance[a].total_cmp(&distance[b]).then(b.cmp(&a)))
        .filter(|&next| distance[next] >= spacing)
    {
        added.push(next);
        let at = points[next].position;
        for (i, p) in points.iter().enumerate() {
            distance[i] = distance[i].min(p.position.distance_xz(at));
        }
    }
    added
}

/// A safe spawn on the floor at `node`, if it is one.
fn candidate(map: &Map, node: u32, center: Vec3) -> Option<Candidate> {
    let p = map.grid.position(node);
    if harmful(map, p) {
        return None;
    }
    let position = match map.check(p) {
        Verdict::Nudged { ground } => Vec3::new(p.x, ground, p.z),
        Verdict::Kept => p,
        Verdict::Dropped(_) => return None,
    };
    facing(map, node, center).map(|yaw| Candidate { position, yaw })
}

fn cell(p: Vec3, size: f32) -> (i32, i32, i32) {
    (
        (p.x / size).floor() as i32,
        (p.y / 2.0).floor() as i32,
        (p.z / size).floor() as i32,
    )
}

/// The distributed points, then the islands' (with the island's name).
type Spawns = Vec<(Candidate, Option<&'static str>)>;

fn generate(map: &Map, plot: &mut File) -> Result<Spawns, String> {
    let seeds: Vec<_> = map
        .data
        .spawns
        .iter()
        .filter_map(|s| map.grid.locate(s.position, 1.0))
        .collect();
    let reachable = map.grid.distances_from(&seeds);
    let (area, _) = area(&map.data.project)?;
    let center = Vec3::new((area[0] + area[2]) * 0.5, 0.0, (area[1] + area[3]) * 0.5);
    let (mut cells, mut island_cells) = (BTreeSet::new(), BTreeSet::new());
    let (mut candidates, mut on_islands) = (Vec::new(), Vec::new());
    let islands = if map.data.project == TRAINING { arena::islands() } else { &[] };
    for node in 0..map.grid.len() as u32 {
        if !reachable.reaches(node) || !map.grid.can_stand(node) {
            continue;
        }
        let p = map.grid.position(node);
        if !in_play_area(map, p) {
            continue;
        }
        if node % 16 == 0 {
            writeln!(plot, "{},{},{}", p.x, p.y, p.z).map_err(|e| e.to_string())?;
        }
        if let Some(island) = islands.iter().find(|i| i.holds(p.x, p.y, p.z))
            && island_cells.insert(cell(p, ISLAND_LATTICE))
            && let Some(c) = candidate(map, node, center)
        {
            on_islands.push((c, island.name));
        }
        // A 3 m lattice is dense enough to find cover without doing expensive
        // collision queries for every quarter-metre grid node. Keep floors apart.
        if cells.insert(cell(p, 3.0))
            && let Some(c) = candidate(map, node, center)
        {
            candidates.push(c);
        }
    }
    eprintln!(
        "{}: {} safe candidates on {} floors, {} on raised islands",
        map.data.project,
        candidates.len(),
        map.grid.len(),
        on_islands.len()
    );
    let mut spawns: Spawns = choose(&candidates, COUNT, MIN_SPACING)?
        .into_iter()
        .map(|i| (candidates[i], None))
        .collect();
    let chosen: Vec<_> = spawns.iter().map(|(c, _)| c.position).collect();
    let island_points: Vec<_> = on_islands.iter().map(|(c, _)| *c).collect();
    for i in extend(&island_points, &chosen, MIN_SPACING) {
        spawns.push((on_islands[i].0, Some(on_islands[i].1)));
    }
    Ok(spawns)
}

fn run() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let (mut layers, mut projects, mut plot_dir) = (Vec::new(), Vec::new(), None);
    while let Some(flag) = args.next() {
        let value = args.next().ok_or_else(|| format!("{flag} needs a value"))?;
        match flag.as_str() {
            "--db" | "--patch" => layers.push(PathBuf::from(value)),
            "--project" => projects.push(value),
            "--plots" => plot_dir = Some(PathBuf::from(value)),
            _ => return Err(format!("unknown flag {flag}")),
        }
    }
    if layers.is_empty() || projects.is_empty() {
        return Err("--db and at least one --project are required".into());
    }
    let plots = plot_dir.ok_or("--plots is required")?;
    std::fs::create_dir_all(&plots).map_err(|e| e.to_string())?;
    let mut resources = Resources::open_layers(&layers).map_err(|e| e.to_string())?;
    let materials = Materials::load(&mut resources).map_err(|e| e.to_string())?;
    println!(
        "project\tpoint\tx\ty\tz\trx\try\trz\tenemy_check_radius\tinvulnerability_time\tverdict\tground\treason"
    );
    for project in projects {
        eprintln!("loading {project}");
        let (bounds, _) = area(&project)?;
        let map = Map::load(&mut resources, &materials, &project, bounds)?;
        let mut plot = File::create(plots.join(project.replace('/', "_") + ".csv"))
            .map_err(|e| e.to_string())?;
        let points = generate(&map, &mut plot)?;
        for (i, (c, island)) in points.iter().enumerate() {
            let p = c.position;
            if harmful(&map, p) || !in_play_area(&map, p) {
                return Err(format!(
                    "{project}: selected point {i} is outside safe play"
                ));
            }
            if let Verdict::Dropped(why) = map.check(p) {
                return Err(format!(
                    "{project}: selected point {i} failed final check: {why}"
                ));
            }
            // Verify the stored engine rotation, independently of the angle
            // sampled above. A mirrored yaw can otherwise face a wall even
            // though the candidate's originally tested direction was clear.
            let dir = mapdata::math::forward(Vec3::new(0.0, c.yaw, 0.0));
            let start = map.grid.locate(p, 0.6).ok_or("spawn lost its floor")?;
            let end = map
                .grid
                .locate(p + dir * FACING_REACH, 0.6)
                .ok_or("spawn's stored facing has no destination")?;
            if !map.grid.can_stand(end)
                || !in_play_area(&map, map.grid.position(end))
                || harmful(&map, map.grid.position(end))
                || !map
                    .grid
                    .walks_straight(start, end, FACING_REACH + 0.5, |n| map.grid.can_stand(n))
            {
                return Err(format!("{project}: point {i}'s stored facing is blocked"));
            }
            let on = island.map_or(String::new(), |name| format!("; raised island {name}"));
            println!(
                "{project}\tported_{i:02}\t{}\t{}\t{}\t0\t{}\t0\t30\t2\tkept\t\treachable standing floor; body clear; harmful zones >=2m; walkable facing 6m; spacing >=12m{on}",
                p.x, p.y, p.z, c.yaw
            );
        }
        eprintln!(
            "{project}: {} checked FFA points, {} added on raised islands",
            points.len(),
            points.iter().filter(|(_, island)| island.is_some()).count()
        );
    }
    Ok(())
}
fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn distributed_selection_is_deterministic_and_never_repeats_a_point() {
        let points: Vec<_> = (0..10)
            .flat_map(|x| {
                (0..10).map(move |z| Candidate {
                    position: Vec3::new(x as f32 * 10.0, 0.0, z as f32 * 10.0),
                    yaw: 0.0,
                })
            })
            .collect();
        let chosen = choose(&points, 24, 12.0).unwrap();
        assert_eq!(chosen, choose(&points, 24, 12.0).unwrap());
        assert_eq!(chosen.len(), 24);
        for (j, &a) in chosen.iter().enumerate() {
            for &b in &chosen[..j] {
                assert!(points[a].position.distance_xz(points[b].position) >= 12.0);
            }
        }
    }
    #[test]
    fn island_points_keep_the_spacing_from_every_point_and_stop_when_none_does() {
        // A 20 m strip: 0 m is taken, so 12 m and then nothing else fits.
        let points: Vec<_> = (0..=20)
            .map(|x| Candidate { position: Vec3::new(x as f32, 20.0, 0.0), yaw: 0.0 })
            .collect();
        let added = extend(&points, &[Vec3::new(0.0, 0.0, 0.0)], 12.0);
        assert_eq!(added, vec![20]);
        assert_eq!(extend(&points, &[], 12.0), vec![0, 20]);
        assert!(extend(&points, &[Vec3::new(10.0, 0.0, 0.0)], 12.0).is_empty());
    }
    #[test]
    fn a_map_too_small_for_safe_spacing_is_refused() {
        assert!(choose(&[], 24, 12.0).is_err());
        let points = [Candidate {
            position: Vec3::new(0.0, 0.0, 0.0),
            yaw: 0.0,
        }; 2];
        assert!(choose(&points, 2, 12.0).is_err());
    }

    #[test]
    fn reviewed_bounds_include_the_warehouses_west_and_limit_the_training_scenery() {
        let (warehouse, clip) = area("v010b_cs_mp_military_1/day_4").unwrap();
        assert!(warehouse[0] < -120.0 && warehouse[2] > 110.0);
        assert!(clip.is_none());
        assert!(arena::contains(20.0, 0.0, 2.0));
        assert!(!arena::contains(20.0, 80.0, 2.0));
        assert!(area("level_03/day_4").is_err());
    }
}
