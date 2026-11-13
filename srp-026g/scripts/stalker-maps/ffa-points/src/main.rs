//! Author FFA points on the three STALKER maps. Explicit TSV output is reviewed
//! and committed, then ../make_ffa_edits.py creates the source edit list.
//! Candidates come from reachable standing floors, pass the same collision
//! checks as imported points, stay clear of harmful zones, and have a six-metre
//! walkable facing. Greedy farthest-point selection distributes 24 points.
//! Training Camp then adds every point its raised islands
//! (training-islands.tsv) take at the same spacing from all of them, in 3D,
//! and every other perimeter point moves to the centre (training-fence.tsv).
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
/// Free for All's own copy of Training Camp (ffa-projects-1.json): no ammo
/// crates, the crater wall. Its points after the 24 are planned on it, and all
/// its points checked on it, so the layers must give it.
const TRAINING_FFA: &str = "v010b_cs_training_camp/day_8";
/// The islands are small: their floors are sampled finer than the 3 m lattice.
const ISLAND_LATTICE: f32 = 0.75;

/// Bounds of the reviewed port's minimaps (stalker-maps@6). The imported
/// projects have a dummy 2 m levelmap; mapdata's native-map fallback truncates
/// Military Warehouses. Training Camp has open scenery beyond its play area.
fn area(project: &str) -> Result<([f32; 4], Option<f32>), String> {
    match project {
        "v010b_cs_mp_bath/day_4" => Ok(([-48.13, -22.87, 34.12, 53.13], None)),
        "v010b_cs_mp_military_1/day_4" => Ok(([-127.47, -71.12, 115.78, 95.38], None)),
        TRAINING | TRAINING_FFA => Ok(([-47.19, -83.47, 82.81, 91.53], None)),
        _ => Err(format!("no reviewed play area for {project}")),
    }
}

fn in_play_area(map: &Map, p: Vec3) -> bool {
    let (bounds, radius) = area(&map.data.project).expect("checked project");
    p.x >= bounds[0]
        && p.z >= bounds[1]
        && p.x <= bounds[2]
        && p.z <= bounds[3]
        && (![TRAINING, TRAINING_FFA].contains(&map.data.project.as_str())
            || arena::contains(p.x, p.z, 2.0))
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

/// Training Camp's spacing after its 24: in 3D when either point stands on an
/// island (the owner, 2026-11-13: a crater-floor spawn under an island does
/// not take it), horizontal otherwise, as for every point.
fn training_distance(a: Vec3, b: Vec3) -> f32 {
    let on_island = |p: Vec3| arena::islands().iter().any(|i| i.holds(p.x, p.y, p.z));
    if on_island(a) || on_island(b) {
        (a - b).length()
    } else {
        a.distance_xz(b)
    }
}

/// Up to `limit` more points after `chosen`: the candidate farthest from every
/// point so far, as long as one keeps `spacing` from them all.
fn extend(
    points: &[Candidate],
    chosen: &[Vec3],
    spacing: f32,
    limit: usize,
    distance_of: impl Fn(Vec3, Vec3) -> f32,
) -> Vec<usize> {
    let mut distance: Vec<f32> = points
        .iter()
        .map(|p| {
            chosen
                .iter()
                .map(|at| distance_of(p.position, *at))
                .fold(f32::INFINITY, f32::min)
        })
        .collect();
    let mut added = Vec::new();
    while added.len() < limit
        && let Some(next) = (0..points.len())
            .max_by(|&a, &b| distance[a].total_cmp(&distance[b]).then(b.cmp(&a)))
            .filter(|&next| distance[next] >= spacing)
    {
        added.push(next);
        let at = points[next].position;
        for (i, p) in points.iter().enumerate() {
            distance[i] = distance[i].min(distance_of(p.position, at));
        }
    }
    added
}

/// Which of Training Camp's distributed points move to the centre (the owner,
/// 2026-11-13: "remove half spawns on perimeter", "put them in the center"):
/// of those on the perimeter, outside the base's fence or against the crater
/// wall, every other one round the crater. With an odd count, the two kept
/// side by side are where the gap would be largest: of the rotations, the one
/// whose longest stretch between kept points is shortest.
fn perimeter_moves(points: &[Vec3]) -> Vec<usize> {
    let mut ring: Vec<usize> = (0..points.len())
        .filter(|&i| !arena::in_centre(points[i].x, points[i].z))
        .collect();
    let [cx, cz] = arena::crater_centre();
    let angle = |i: usize| (points[i].z - cz).atan2(points[i].x - cx);
    ring.sort_by(|&a, &b| angle(a).total_cmp(&angle(b)));
    let n = ring.len();
    // Rotation r keeps r, r + 2, ... round the ring, in order.
    let kept = |r: usize| -> Vec<usize> {
        (0..n.div_ceil(2)).map(|k| ring[(r + 2 * k) % n]).collect()
    };
    let longest = |r: usize| {
        let k = kept(r);
        (0..k.len())
            .map(|j| points[k[j]].distance_xz(points[k[(j + 1) % k.len()]]))
            .fold(0.0, f32::max)
    };
    let Some(best) = (0..n).min_by(|&a, &b| longest(a).total_cmp(&longest(b)).then(a.cmp(&b)))
    else {
        return Vec::new();
    };
    let keep: BTreeSet<_> = kept(best).into_iter().collect();
    let mut moved: Vec<_> = ring.iter().copied().filter(|i| !keep.contains(i)).collect();
    moved.sort();
    moved
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

/// Where a point comes from.
#[derive(Clone, Copy)]
enum Origin {
    Distributed,
    /// A Training Camp perimeter point's id, placed in the centre.
    Moved,
    Island(&'static str),
}

/// The distributed points (Training Camp's moved ones in their place), then
/// the islands'.
type Spawns = Vec<(Candidate, Origin)>;

/// A map's safe candidates: on the 3 m lattice, and (Training Camp) on its
/// islands' finer one. `plot` samples the standing floors.
type Found = (Vec<Candidate>, Vec<(Candidate, &'static str)>);

fn candidates(map: &Map, mut plot: Option<&mut File>) -> Result<Found, String> {
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
    let islands = if map.data.project == TRAINING_FFA { arena::islands() } else { &[] };
    for node in 0..map.grid.len() as u32 {
        if !reachable.reaches(node) || !map.grid.can_stand(node) {
            continue;
        }
        let p = map.grid.position(node);
        if !in_play_area(map, p) {
            continue;
        }
        if node % 16 == 0
            && let Some(plot) = plot.as_mut()
        {
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
    Ok((candidates, on_islands))
}

fn generate(map: &Map, plot: &mut File) -> Result<Spawns, String> {
    let (candidates, _) = candidates(map, Some(plot))?;
    Ok(choose(&candidates, COUNT, MIN_SPACING)?
        .into_iter()
        .map(|i| (candidates[i], Origin::Distributed))
        .collect())
}

/// Training Camp after its 24, on Free for All's own project: half its
/// perimeter points leave, its islands take what they can, then the leavers'
/// ids go to the centre.
fn training(ffa: &Map, mut spawns: Spawns) -> Result<Spawns, String> {
    let (candidates, on_islands) = candidates(ffa, None)?;
    let positions: Vec<_> = spawns.iter().map(|(c, _)| c.position).collect();
    let moved = perimeter_moves(&positions);
    let mut kept: Vec<_> = (0..positions.len())
        .filter(|i| !moved.contains(i))
        .map(|i| positions[i])
        .collect();
    let island_points: Vec<_> = on_islands.iter().map(|(c, _)| *c).collect();
    let mut islands_taken = Vec::new();
    for i in extend(&island_points, &kept, MIN_SPACING, usize::MAX, training_distance) {
        kept.push(island_points[i].position);
        islands_taken.push((on_islands[i].0, Origin::Island(on_islands[i].1)));
    }
    let centre: Vec<_> = candidates
        .iter()
        .chain(&island_points)
        .copied()
        .filter(|c| arena::in_centre(c.position.x, c.position.z))
        .collect();
    let placed = extend(&centre, &kept, MIN_SPACING, moved.len(), training_distance);
    if placed.len() < moved.len() {
        return Err(format!(
            "only {} of {} perimeter points fit in the centre",
            placed.len(),
            moved.len()
        ));
    }
    for (&id, &i) in moved.iter().zip(&placed) {
        spawns[id] = (centre[i], Origin::Moved);
    }
    spawns.extend(islands_taken);
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
        let mut points = generate(&map, &mut plot)?;
        // Training Camp's points are Free for All's own project's: planned
        // after the 24 and checked there.
        let map = if project == TRAINING {
            let ffa = Map::load(&mut resources, &materials, TRAINING_FFA, bounds)?;
            points = training(&ffa, points)?;
            ffa
        } else {
            map
        };
        for (i, (c, origin)) in points.iter().enumerate() {
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
            let on = match origin {
                Origin::Distributed => String::new(),
                Origin::Moved => "; moved from the perimeter to the centre, spacing in 3D from islands".into(),
                Origin::Island(name) => format!("; raised island {name}, spacing in 3D"),
            };
            println!(
                "{project}\tported_{i:02}\t{}\t{}\t{}\t0\t{}\t0\t30\t2\tkept\t\treachable standing floor; body clear; harmful zones >=2m; walkable facing 6m; spacing >=12m{on}",
                p.x, p.y, p.z, c.yaw
            );
        }
        let count = |of: fn(&Origin) -> bool| points.iter().filter(|(_, o)| of(o)).count();
        eprintln!(
            "{project}: {} checked FFA points, {} moved to the centre, {} added on raised islands",
            points.len(),
            count(|o| matches!(o, Origin::Moved)),
            count(|o| matches!(o, Origin::Island(_)))
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
        // A 20 m strip at y 20: its 0 m end is taken, so 12 m and then nothing
        // else fits.
        let points: Vec<_> = (0..=20)
            .map(|x| Candidate { position: Vec3::new(x as f32, 20.0, 0.0), yaw: 0.0 })
            .collect();
        let d3 = |a: Vec3, b: Vec3| (a - b).length();
        let all = usize::MAX;
        assert_eq!(extend(&points, &[Vec3::new(0.0, 20.0, 0.0)], 12.0, all, d3), vec![20]);
        assert_eq!(extend(&points, &[], 12.0, all, d3), vec![0, 20]);
        assert_eq!(extend(&points, &[], 12.0, 1, d3), vec![0]);
        assert!(extend(&points, &[Vec3::new(10.0, 20.0, 0.0)], 12.0, all, d3).is_empty());
        // A spawn 20 m below the strip's middle takes it in plan, not in 3D.
        let below = [Vec3::new(10.0, 0.0, 0.0)];
        assert_eq!(extend(&points, &below, 12.0, all, d3), vec![0, 20]);
        assert!(extend(&points, &below, 12.0, all, Vec3::distance_xz).is_empty());
    }
    #[test]
    fn every_other_perimeter_point_moves_and_the_extra_one_stays_in_the_widest_gap() {
        // Fifteen points round the crater, beyond its wall, one of them (the
        // last) far from its neighbours.
        let [cx, cz] = arena::crater_centre();
        let points: Vec<_> = (0..15)
            .map(|i| {
                let a = if i == 14 { 340.0f32 } else { i as f32 * 22.0 }.to_radians();
                Vec3::new(cx + 90.0 * a.cos(), 0.0, cz + 90.0 * a.sin())
            })
            .collect();
        let moved = perimeter_moves(&points);
        assert_eq!(moved.len(), 7);
        // Two neighbours kept side by side, around the widest gap (13 to 14).
        assert_eq!(moved, vec![0, 2, 4, 6, 8, 10, 12]);
        // A centre point never moves.
        let mut with_centre = points.clone();
        with_centre.push(Vec3::new(20.0, 0.0, 0.0));
        assert!(!perimeter_moves(&with_centre).contains(&15));
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
