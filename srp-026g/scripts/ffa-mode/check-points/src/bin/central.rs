//! Author extra London FFA spawns in its central play area. Existing solo
//! spawns are exclusion points: new points stay >=12m from them and each other.
//! Output is reviewed and committed (the region's TSV), then made into source
//! edits by make_london_edits.py. Uses actual layered collision/navigation data.
//!
//! `--region central` (the default, 2026-11-12): six, on level_09/day_4 with
//! the original 22 solo spawns, 12 m from them in plan: london-center.tsv.
//! `--region underpass` (the owner, 2026-11-14: "put more points in basement
//! there"): four in the underpass under the roundabout (a floor whose ceiling
//! is below the street's 6 m: its plate at 4.73 over a floor at 0.7), on
//! level_09/day_8 as every list before ffa-london-underpass-1 leaves it (28
//! solo spawns), 12 m from them in 3D: the rotunda's storeys over it are 13 m
//! up, the street 5 m: london-underpass.tsv.
use ffa_check_points::{Map, Verdict};
use mapdata::{Materials, Resources, Vec3};
use std::{collections::BTreeSet, fs::File, io::Write, path::PathBuf, process::ExitCode};

const AREA: [f32; 4] = [-35.0, -30.0, 40.0, 30.0];
const MIN_SPACING: f32 = 12.0;
const HAZARD_MARGIN: f32 = 2.0;
const FACING_REACH: f32 = 6.0;
/// How far ahead a facing looks.
const SIGHT: f32 = 20.0;
/// The street round the roundabout: a floor whose ceiling is below it is
/// the underpass's.
const STREET: f32 = 6.0;

/// Where the new points go, on which project, and how far apart.
#[derive(Clone, Copy)]
enum Region {
    /// Central London's open floors; spacing in plan.
    Central,
    /// The underpass's floors under the roundabout; spacing in 3D.
    Underpass,
}

impl Region {
    fn parse(name: &str) -> Result<Self, String> {
        match name {
            "central" => Ok(Self::Central),
            "underpass" => Ok(Self::Underpass),
            _ => Err(format!("unknown region {name}")),
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::Central => "central",
            Self::Underpass => "underpass",
        }
    }

    fn project(self) -> &'static str {
        match self {
            Self::Central => "level_09/day_4",
            Self::Underpass => "level_09/day_8",
        }
    }

    /// The solo spawns the project has before these are added.
    fn existing(self) -> usize {
        match self {
            Self::Central => 22,
            Self::Underpass => 28,
        }
    }

    fn count(self) -> usize {
        match self {
            Self::Central => 6,
            Self::Underpass => 4,
        }
    }

    /// The exclusion radius they are given (ffa-radius-1 retunes the
    /// central ones, made before it, to 20 m).
    fn radius(self) -> u32 {
        match self {
            Self::Central => 30,
            Self::Underpass => 20,
        }
    }

    fn holds(self, map: &Map, node: u32) -> bool {
        let p = map.grid.position(node);
        let central = p.x >= AREA[0] && p.z >= AREA[1] && p.x <= AREA[2] && p.z <= AREA[3];
        match self {
            Self::Central => central,
            Self::Underpass => central && p.y + map.grid.headroom(node) < STREET,
        }
    }

    /// Whether a spawn may face the floor `end` 6 m ahead, seeing `sight`
    /// metres: the underpass's look along it (its floor ahead, nothing within
    /// SIGHT), not out of it or into a nook.
    fn faces(self, map: &Map, end: u32, sight: f32) -> bool {
        match self {
            Self::Central => true,
            Self::Underpass => self.holds(map, end) && sight >= SIGHT,
        }
    }

    fn distance(self, a: Vec3, b: Vec3) -> f32 {
        match self {
            Self::Central => a.distance_xz(b),
            Self::Underpass => (a - b).length(),
        }
    }

    fn reason(self) -> &'static str {
        match self {
            Self::Central => "inside central London; at least 12m from existing solo spawns",
            Self::Underpass => {
                "in the underpass, under its ceiling, facing along it; at least 12m in 3D from existing solo spawns"
            }
        }
    }
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

fn facing(region: Region, map: &Map, node: u32, center: Vec3) -> Option<f32> {
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
            let sight = map.rays.ray_cast(eye, dir, SIGHT).unwrap_or(SIGHT);
            if !map.grid.can_stand(end)
                || !region.faces(map, end, sight)
                || (at.y - p.y).abs() > 0.6
                || harmful(map, at)
                || !map
                    .grid
                    .walks_straight(node, end, FACING_REACH + 0.5, |n| map.grid.can_stand(n))
            {
                return None;
            }
            // Prefer room to see ahead, then look into the play area on a tie.
            Some((yaw, sight, dir.x * toward.x + dir.z * toward.z))
        })
        .max_by(|a, b| a.1.total_cmp(&b.1).then(a.2.total_cmp(&b.2)))
        .map(|(yaw, _, _)| yaw)
}

fn choose(
    region: Region,
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
                .map(|at| region.distance(p.position, *at))
                .fold(f32::INFINITY, f32::min)
        })
        .collect();
    while chosen.len() < count {
        let next = (0..points.len())
            .max_by(|&a, &b| distance[a].total_cmp(&distance[b]).then(b.cmp(&a)))
            .unwrap();
        if distance[next] < spacing {
            return Err(format!(
                "only {} {} points fit at {spacing} m spacing",
                chosen.len(),
                region.name()
            ));
        }
        chosen.push(next);
        for (i, p) in points.iter().enumerate() {
            distance[i] = distance[i].min(region.distance(p.position, points[next].position));
        }
    }
    Ok(chosen)
}

fn generate(
    region: Region,
    map: &Map,
    existing: &[Vec3],
    plot: &mut File,
) -> Result<Vec<Candidate>, String> {
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
        if !region.holds(map, node) {
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
        if existing
            .iter()
            .any(|at| region.distance(*at, p) < MIN_SPACING)
            || harmful(map, p)
            || matches!(map.check(p), Verdict::Dropped(_))
        {
            continue;
        }
        let position = match map.check(p) {
            Verdict::Nudged { ground } => Vec3::new(p.x, ground, p.z),
            _ => p,
        };
        if let Some(yaw) = facing(region, map, node, center) {
            candidates.push(Candidate { position, yaw });
        }
    }
    eprintln!(
        "{}: {} safe candidates on {} floors",
        map.data.project,
        candidates.len(),
        map.grid.len()
    );
    choose(region, &candidates, existing, region.count(), MIN_SPACING)
        .map(|chosen| chosen.into_iter().map(|i| candidates[i]).collect())
}

fn run() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let (mut layers, mut projects, mut plot_dir) = (Vec::new(), Vec::new(), None);
    let mut region = Region::Central;
    while let Some(flag) = args.next() {
        let value = args.next().ok_or_else(|| format!("{flag} needs a value"))?;
        match flag.as_str() {
            "--db" | "--patch" => layers.push(PathBuf::from(value)),
            "--project" => projects.push(value),
            "--region" => region = Region::parse(&value)?,
            "--plots" => plot_dir = Some(PathBuf::from(value)),
            _ => return Err(format!("unknown flag {flag}")),
        }
    }
    if layers.is_empty() || projects.is_empty() {
        return Err("--db and at least one --project are required".into());
    }
    if let Some(other) = projects.iter().find(|p| *p != region.project()) {
        return Err(format!(
            "{other}: the {} region is authored on London's {}",
            region.name(),
            region.project()
        ));
    }
    let plots = plot_dir.ok_or("--plots is required")?;
    std::fs::create_dir_all(&plots).map_err(|e| e.to_string())?;
    let mut resources = Resources::open_layers(&layers).map_err(|e| e.to_string())?;
    let materials = Materials::load(&mut resources).map_err(|e| e.to_string())?;
    println!(
        "project\tpoint\tx\ty\tz\trx\try\trz\tenemy_check_radius\tinvulnerability_time\tverdict\tground\treason"
    );
    for project in projects {
        eprintln!("loading {project} (extra {} spawns)", region.name());
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
        if existing.len() != region.existing() {
            return Err(format!(
                "expected London's {} FFA spawns, got {}",
                region.existing(),
                existing.len()
            ));
        }
        let points = generate(region, &map, &existing, &mut plot)?;
        for (i, c) in points.iter().enumerate() {
            let p = c.position;
            if harmful(&map, p) || matches!(map.check(p), Verdict::Dropped(_)) {
                return Err(format!("{project}: selected point {i} failed final check"));
            }
            println!(
                "{project}\t{}_{i:02}\t{}\t{}\t{}\t0\t{}\t0\t{}\t2\tkept\t\treachable standing floor; body clear; harmful zones >=2m; walkable facing 6m; spacing >=12m; {}",
                region.name(),
                p.x,
                p.y,
                p.z,
                c.yaw,
                region.radius(),
                region.reason()
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
        let central = Region::Central;
        let chosen = choose(central, &points, &existing, 3, 12.0).unwrap();
        assert_eq!(
            chosen,
            choose(central, &points, &existing, 3, 12.0).unwrap()
        );
        assert!(!chosen.contains(&0));
        for (i, &a) in chosen.iter().enumerate() {
            assert!(points[a].position.distance_xz(existing[0]) >= 12.0);
            for &b in &chosen[..i] {
                assert!(points[a].position.distance_xz(points[b].position) >= 12.0);
            }
        }
        assert!(choose(central, &points, &existing, 4, 12.0).is_err());
        assert!(choose(central, &points, &[], 1, 12.0).is_err());
    }

    /// Under a spawn of the storey 13 m above, the underpass has room; in
    /// plan it would not.
    #[test]
    fn the_underpass_measures_in_3d() {
        let points = [point(0.0, 0.0)];
        let above = [Vec3::new(0.0, 13.0, 0.0)];
        assert_eq!(
            choose(Region::Underpass, &points, &above, 1, 12.0),
            Ok(vec![0])
        );
        assert!(choose(Region::Central, &points, &above, 1, 12.0).is_err());
        let street = [Vec3::new(8.0, 5.3, 0.0)];
        assert!(choose(Region::Underpass, &points, &street, 1, 12.0).is_err());
    }
}
