//! Author FFA points on maps with no 0.31 donor. Explicit TSV output is reviewed
//! and committed, then make_ported_edits.py creates the source edit list.
//! Candidates come from reachable standing floors, pass the same collision
//! checks as imported points, stay clear of harmful zones, and have a six-metre
//! walkable facing. Candidates stay within 12 m of an established spawn footprint
//! (the corresponding 0.26 FFA layout, or the same map's authored team spawns).
//! Greedy farthest-point selection distributes 22 points.
use ffa_check_points::{Map, Verdict};
use mapdata::{Materials, Resources, Vec3};
use std::{collections::BTreeSet, fs::File, io::Write, path::PathBuf, process::ExitCode};

const COUNT: usize = 22;
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

/// `count` of `points`, spread: the one nearest the middle, then each the
/// farthest from all chosen (no minimum distance: the owner, 2026-11-14, of the
/// 12 m spacing, "we don't need this rules at all").
fn choose(points: &[Candidate], count: usize) -> Result<Vec<usize>, String> {
    if points.len() < count {
        return Err(format!("{} safe standing candidates for {count} points", points.len()));
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
    distance[first] = f32::NEG_INFINITY;
    while chosen.len() < count {
        let last = points[*chosen.last().unwrap()].position;
        for (i, p) in points.iter().enumerate() {
            distance[i] = distance[i].min(p.position.distance_xz(last));
        }
        let next = (0..points.len())
            .max_by(|&a, &b| distance[a].total_cmp(&distance[b]).then(b.cmp(&a)))
            .unwrap();
        distance[next] = f32::NEG_INFINITY;
        chosen.push(next);
    }
    Ok(chosen)
}

fn generate(map: &Map, anchors: &[Vec3], plot: &mut File) -> Result<Vec<Candidate>, String> {
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
        // A 3 m lattice is dense enough to find cover without doing expensive
        // collision queries for every quarter-metre grid node. Keep floors apart.
        if !cells.insert((
            (p.x / 3.0).floor() as i32,
            (p.y / 2.0).floor() as i32,
            (p.z / 3.0).floor() as i32,
        )) {
            continue;
        }
        if !anchors
            .iter()
            .any(|a| a.distance_xz(p) <= 12.0 && (a.y - p.y).abs() <= 5.0)
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
    choose(&candidates, COUNT)
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
                let (target, source) = value
                    .split_once('=')
                    .ok_or("--project expects target=spawn-source")?;
                projects.push((target.to_owned(), source.to_owned()));
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
    for (project, source) in projects {
        eprintln!("loading {project} (spawn footprint {source})");
        let map = Map::load(&mut resources, &materials, &project)?;
        let mut plot = File::create(plots.join(project.replace('/', "_") + ".csv"))
            .map_err(|e| e.to_string())?;
        let config = resources
            .config(&format!("projects/{source}/client_project"))
            .map_err(|e| e.to_string())?;
        let table = config
            .get("server_objects")
            .and_then(|v| v.get("respawn_points"))
            .ok_or("no source spawn points")?;
        let mut anchors = Vec::new();
        for point in table.children() {
            let team = point.i32_at("team").map_err(|e| e.to_string())?;
            if (source == project && matches!(team, 0 | 1)) || (source != project && team == 2) {
                anchors.push(Vec3::from_array(
                    point.float3_at("position").map_err(|e| e.to_string())?,
                ));
            }
        }
        if anchors.is_empty() {
            return Err(format!("{source}: no appropriate spawn footprint"));
        }
        let points = generate(&map, &anchors, &mut plot)?;
        for (i, c) in points.iter().enumerate() {
            let p = c.position;
            if harmful(&map, p) || matches!(map.check(p), Verdict::Dropped(_)) {
                return Err(format!("{project}: selected point {i} failed final check"));
            }
            println!(
                "{project}\tported_{i:02}\t{}\t{}\t{}\t0\t{}\t0\t30\t2\tkept\t\treachable standing floor; body clear; harmful zones >=2m; walkable facing 6m; spread farthest-first; inside established spawn footprint",
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
        let chosen = choose(&points, 24).unwrap();
        assert_eq!(chosen, choose(&points, 24).unwrap());
        assert_eq!(chosen.len(), 24);
        for (j, &a) in chosen.iter().enumerate() {
            for &b in &chosen[..j] {
                assert!(points[a].position.distance_xz(points[b].position) >= 12.0);
            }
        }
    }
    #[test]
    fn only_too_few_candidates_are_refused_points_may_stand_close() {
        assert!(choose(&[], 24).is_err());
        let points = [Candidate {
            position: Vec3::new(0.0, 0.0, 0.0),
            yaw: 0.0,
        }; 2];
        assert_eq!(choose(&points, 2).unwrap(), [0, 1]);
        assert!(choose(&points, 3).is_err());
    }
}
