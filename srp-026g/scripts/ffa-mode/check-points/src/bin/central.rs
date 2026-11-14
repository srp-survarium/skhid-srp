//! Author six extra London FFA spawns in its central play area. Existing solo
//! spawns are exclusion points: new points stay >=12m from them and each other.
//! Output is reviewed and committed as london-center.tsv, then made into source
//! edits by make_london_edits.py. Uses actual layered collision/navigation data.
use ffa_check_points::{Map, Verdict};
use mapdata::{Materials, Resources, Vec3};
use std::{collections::BTreeSet, fs::File, io::Write, path::PathBuf, process::ExitCode};

const COUNT: usize = 6;
const AREA: [f32; 4] = [-35.0, -30.0, 40.0, 30.0];
const MIN_SPACING: f32 = 12.0;
const HAZARD_MARGIN: f32 = 2.0;
const FACING_REACH: f32 = 6.0;

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
                || !map
                    .grid
                    .walks_straight(node, end, FACING_REACH + 0.5, |n| map.grid.can_stand(n))
            {
                return None;
            }
            let sight = map.rays.ray_cast(eye, dir, 20.0).unwrap_or(20.0);
            // Prefer room to see ahead, then look into the play area on a tie.
            Some((yaw, sight, dir.x * toward.x + dir.z * toward.z))
        })
        .max_by(|a, b| a.1.total_cmp(&b.1).then(a.2.total_cmp(&b.2)))
        .map(|(yaw, _, _)| yaw)
}

fn choose(
    points: &[Candidate],
    existing: &[Vec3],
    count: usize,
    spacing: f32,
) -> Result<Vec<usize>, String> {
    if points.is_empty() || existing.is_empty() {
        return Err("safe candidates and existing FFA spawns are required".into());
    }
    let mut chosen = Vec::new();
    let mut distance: Vec<_> = points
        .iter()
        .map(|p| {
            existing
                .iter()
                .map(|at| p.position.distance_xz(*at))
                .fold(f32::INFINITY, f32::min)
        })
        .collect();
    while chosen.len() < count {
        let next = (0..points.len())
            .max_by(|&a, &b| distance[a].total_cmp(&distance[b]).then(b.cmp(&a)))
            .unwrap();
        if distance[next] < spacing {
            return Err(format!(
                "only {} central points fit at {spacing} m spacing",
                chosen.len()
            ));
        }
        chosen.push(next);
        for (i, p) in points.iter().enumerate() {
            distance[i] = distance[i].min(p.position.distance_xz(points[next].position));
        }
    }
    Ok(chosen)
}

fn generate(map: &Map, existing: &[Vec3], plot: &mut File) -> Result<Vec<Candidate>, String> {
    let seeds: Vec<_> = map
        .data
        .spawns
        .iter()
        .filter_map(|s| map.grid.locate(s.position, 1.0))
        .collect();
    let reachable = map.grid.distances_from(&seeds);
    let area = map.data.play_area(0.0);
    let center = Vec3::new((area[0] + area[2]) * 0.5, 0.0, (area[1] + area[3]) * 0.5);
    let mut cells = BTreeSet::new();
    let mut candidates = Vec::new();
    for node in 0..map.grid.len() as u32 {
        if !reachable.reaches(node) || !map.grid.can_stand(node) {
            continue;
        }
        let p = map.grid.position(node);
        if node % 16 == 0 {
            writeln!(plot, "{},{},{}", p.x, p.y, p.z).map_err(|e| e.to_string())?;
        }
        if p.x < AREA[0] || p.z < AREA[1] || p.x > AREA[2] || p.z > AREA[3] {
            continue;
        }
        // Sample central standing floors every two metres, keeping levels apart.
        if !cells.insert((
            (p.x / 2.0).floor() as i32,
            (p.y / 2.0).floor() as i32,
            (p.z / 2.0).floor() as i32,
        )) {
            continue;
        }
        if existing.iter().any(|at| at.distance_xz(p) < MIN_SPACING)
            || harmful(map, p)
            || matches!(map.check(p), Verdict::Dropped(_))
        {
            continue;
        }
        let position = match map.check(p) {
            Verdict::Nudged { ground } => Vec3::new(p.x, ground, p.z),
            _ => p,
        };
        if let Some(yaw) = facing(map, node, center) {
            candidates.push(Candidate { position, yaw });
        }
    }
    eprintln!(
        "{}: {} safe candidates on {} floors",
        map.data.project,
        candidates.len(),
        map.grid.len()
    );
    choose(&candidates, existing, COUNT, MIN_SPACING)
        .map(|chosen| chosen.into_iter().map(|i| candidates[i]).collect())
}

fn run() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let (mut layers, mut projects, mut plot_dir) = (Vec::new(), Vec::new(), None);
    while let Some(flag) = args.next() {
        let value = args.next().ok_or_else(|| format!("{flag} needs a value"))?;
        match flag.as_str() {
            "--db" | "--patch" => layers.push(PathBuf::from(value)),
            "--project" => {
                if value != "level_09/day_4" {
                    return Err("this authoring region is London's level_09/day_4".into());
                }
                projects.push(value);
            }
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
        eprintln!("loading {project} (extra central spawns)");
        let map = Map::load(&mut resources, &materials, &project)?;
        let mut plot = File::create(plots.join(project.replace('/', "_") + ".csv"))
            .map_err(|e| e.to_string())?;
        let existing: Vec<_> = map
            .data
            .spawns
            .iter()
            .filter(|s| s.team == 2)
            .map(|s| s.position)
            .collect();
        if existing.len() != 22 {
            return Err(format!(
                "expected London's original 22 FFA spawns, got {}",
                existing.len()
            ));
        }
        let points = generate(&map, &existing, &mut plot)?;
        for (i, c) in points.iter().enumerate() {
            let p = c.position;
            if harmful(&map, p) || matches!(map.check(p), Verdict::Dropped(_)) {
                return Err(format!("{project}: selected point {i} failed final check"));
            }
            println!(
                "{project}\tcentral_{i:02}\t{}\t{}\t{}\t0\t{}\t0\t30\t2\tkept\t\treachable standing floor; body clear; harmful zones >=2m; walkable facing 6m; spacing >=12m; inside central London; at least 12m from existing solo spawns",
                p.x, p.y, p.z, c.yaw
            );
        }
        eprintln!("{project}: {} checked FFA points", points.len());
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
    fn point(x: f32, z: f32) -> Candidate {
        Candidate {
            position: Vec3::new(x, 0.0, z),
            yaw: 0.0,
        }
    }
    #[test]
    fn additions_keep_clear_of_existing_spawns_and_each_other() {
        let points = [
            point(1.0, 0.0),
            point(13.0, 0.0),
            point(26.0, 0.0),
            point(13.0, 13.0),
        ];
        let existing = [Vec3::new(0.0, 0.0, 0.0)];
        let chosen = choose(&points, &existing, 3, 12.0).unwrap();
        assert_eq!(chosen, choose(&points, &existing, 3, 12.0).unwrap());
        assert!(!chosen.contains(&0));
        for (i, &a) in chosen.iter().enumerate() {
            assert!(points[a].position.distance_xz(existing[0]) >= 12.0);
            for &b in &chosen[..i] {
                assert!(points[a].position.distance_xz(points[b].position) >= 12.0);
            }
        }
        assert!(choose(&points, &existing, 4, 12.0).is_err());
        assert!(choose(&points, &[], 1, 12.0).is_err());
    }
}
