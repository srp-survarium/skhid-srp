//! Author FFA points on the three STALKER maps. Explicit TSV output is reviewed
//! and committed, then ../make_ffa_edits.py creates the source edit list.
//! Candidates come from reachable standing floors, pass the same collision
//! checks as imported points, stay clear of harmful zones, and have a six-metre
//! walkable facing. Greedy farthest-point selection distributes 24 points.
//! Training Camp's points are not planned again: those of the committed table
//! (--points) that are still fit on Free for All's own project keep their
//! place and id, and the others' ids take new places inside its base's fence
//! (training-fence.tsv), spaced in 3D from its raised islands
//! (training-islands.tsv).
mod collision;
mod arena;
use collision::{Map, Verdict};
use mapdata::{Materials, Resources, Vec3};
use std::{
    collections::BTreeSet,
    fs::File,
    io::Write,
    path::{Path, PathBuf},
    process::ExitCode,
};

const COUNT: usize = 24;
const MIN_SPACING: f32 = 12.0;
const HAZARD_MARGIN: f32 = 2.0;
const FACING_REACH: f32 = 6.0;
/// Training Camp as the table and the edit generator name it: its Team
/// Deathmatch project, which is never loaded or edited here.
const TRAINING: &str = "v010b_cs_training_camp/day_4";
/// Free for All's own copy of Training Camp (ffa-projects-1.json): no ammo
/// crates, the walker wall behind its base's fence, a truck in each gateway.
/// Its points are checked and placed on it, so the layers must give it.
const TRAINING_FFA: &str = "v010b_cs_training_camp/day_8";
/// The islands are small: their floors are sampled finer than the 3 m lattice.
const ISLAND_LATTICE: f32 = 0.75;
/// A new Training Camp place and its facing's end stay this far inside the fence.
const FENCE_MARGIN: f32 = 2.0;
/// What every row's reason starts with: the checks every point passes.
const REASON: &str = "reachable standing floor; body clear; harmful zones >=2m; walkable facing 6m; spacing >=12m";

/// Bounds of the reviewed port's minimaps (stalker-maps@6). The imported
/// projects have a dummy 2 m levelmap; mapdata's native-map fallback truncates
/// Military Warehouses. Training Camp has open scenery beyond its fence.
fn area(project: &str) -> Result<([f32; 4], Option<f32>), String> {
    match project {
        "v010b_cs_mp_bath/day_4" => Ok(([-48.13, -22.87, 34.12, 53.13], None)),
        "v010b_cs_mp_military_1/day_4" => Ok(([-127.47, -71.12, 115.78, 95.38], None)),
        TRAINING_FFA => Ok(([-47.19, -83.47, 82.81, 91.53], None)),
        _ => Err(format!("no reviewed play area for {project}")),
    }
}

/// Whether a point is on the map's playable ground: Training Camp's is inside
/// its base's fence, `fence_margin` from it.
fn in_play_area(map: &Map, p: Vec3, fence_margin: f32) -> bool {
    let (bounds, radius) = area(&map.data.project).expect("checked project");
    p.x >= bounds[0]
        && p.z >= bounds[1]
        && p.x <= bounds[2]
        && p.z <= bounds[3]
        && (map.data.project != TRAINING_FFA || arena::fenced(p.x, p.z, fence_margin))
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
                || !in_play_area(map, at, FENCE_MARGIN)
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

/// Training Camp's spacing: in 3D when either point stands on an island (the
/// owner, 2026-11-13: a spawn on the ground under an island does not take
/// it), horizontal otherwise, as for every point.
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

/// Fills the empty slots, lowest first, from `fresh`. When it runs out, the
/// last slot moves down into the lowest empty one until none is empty, so the
/// ids stay 0..n without a gap and as few change as can. Returns the slots and,
/// for each that moved down, its (new, old) id.
fn refill<T>(
    mut slots: Vec<Option<T>>,
    fresh: impl IntoIterator<Item = T>,
) -> (Vec<T>, Vec<(usize, usize)>) {
    let mut fresh = fresh.into_iter();
    for slot in slots.iter_mut().filter(|slot| slot.is_none()) {
        match fresh.next() {
            Some(value) => *slot = Some(value),
            None => break,
        }
    }
    let mut moved_down = Vec::new();
    loop {
        while slots.last().is_some_and(Option::is_none) {
            slots.pop();
        }
        let Some(empty) = slots.iter().position(Option::is_none) else {
            break;
        };
        // The last slot is filled and comes after the empty one.
        moved_down.push((empty, slots.len() - 1));
        slots[empty] = slots.pop().flatten();
    }
    (slots.into_iter().flatten().collect(), moved_down)
}

/// Whether a point is fit on the map as it is: on safe playable ground, a
/// body standing clear on a reachable floor, its stored facing walkable.
fn fit(map: &Map, c: &Candidate) -> Result<(), String> {
    let p = c.position;
    if harmful(map, p) || !in_play_area(map, p, 0.0) {
        return Err("is outside safe play".into());
    }
    if let Verdict::Dropped(why) = map.check(p) {
        return Err(format!("failed the body check: {why}"));
    }
    // Verify the stored engine rotation, independently of the angle sampled
    // for it. A mirrored yaw can otherwise face a wall even though the
    // candidate's originally tested direction was clear.
    let dir = mapdata::math::forward(Vec3::new(0.0, c.yaw, 0.0));
    let start = map.grid.locate(p, 0.6).ok_or("lost its floor")?;
    let end = map
        .grid
        .locate(p + dir * FACING_REACH, 0.6)
        .ok_or("has no floor at its stored facing's end")?;
    if !map.grid.can_stand(end)
        || !in_play_area(map, map.grid.position(end), 0.0)
        || harmful(map, map.grid.position(end))
        || !map
            .grid
            .walks_straight(start, end, FACING_REACH + 0.5, |n| map.grid.can_stand(n))
    {
        return Err("has its stored facing blocked".into());
    }
    Ok(())
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

/// A map's points in id order, each with what its row's reason says after
/// [`REASON`].
type Spawns = Vec<(Candidate, String)>;

/// A map's safe candidates: on the 3 m lattice, and (Training Camp) on its
/// islands' finer one. `plot` samples the standing floors.
type Found = (Vec<Candidate>, Vec<Candidate>);

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
        if !in_play_area(map, p, FENCE_MARGIN) {
            continue;
        }
        if node % 16 == 0
            && let Some(plot) = plot.as_mut()
        {
            writeln!(plot, "{},{},{}", p.x, p.y, p.z).map_err(|e| e.to_string())?;
        }
        if islands.iter().any(|i| i.holds(p.x, p.y, p.z))
            && island_cells.insert(cell(p, ISLAND_LATTICE))
            && let Some(c) = candidate(map, node, center)
        {
            on_islands.push(c);
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
        .map(|i| (candidates[i], String::new()))
        .collect())
}

/// Training Camp's rows of the committed table, in id order.
fn read_table(path: &Path) -> Result<Spawns, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    text.lines()
        .filter(|line| line.split('\t').next() == Some(TRAINING))
        .enumerate()
        .map(|(id, line)| {
            let cells: Vec<&str> = line.split('\t').collect();
            let [_, point, x, y, z, _, yaw, _, _, _, _, _, reason] = cells[..] else {
                return Err(format!("{TRAINING} row {id}: 13 cells expected"));
            };
            let number = |cell: &str| {
                cell.parse::<f32>()
                    .map_err(|e| format!("{TRAINING} row {id}: {cell}: {e}"))
            };
            if point != format!("ported_{id:02}") {
                return Err(format!("{TRAINING} row {id} is {point}: rows are in id order"));
            }
            let note = reason
                .strip_prefix(REASON)
                .ok_or_else(|| format!("{TRAINING} row {id}: its reason lacks the checks"))?;
            let position = Vec3::new(number(x)?, number(y)?, number(z)?);
            Ok((Candidate { position, yaw: number(yaw)? }, note.to_owned()))
        })
        .collect()
}

/// What the row of a point placed anew says: where it went.
fn moved_note(p: Vec3) -> String {
    match arena::islands().iter().find(|i| i.holds(p.x, p.y, p.z)) {
        Some(island) => format!(
            "; moved inside the fence, onto raised island {}, spacing in 3D",
            island.name
        ),
        None => "; moved inside the fence, spacing in 3D from islands".to_owned(),
    }
}

/// Training Camp on Free for All's own project as the layers give it: the
/// table's points that are still fit keep their place and id. The others'
/// ids take new places inside the fence (the ground, the towers, the
/// islands): the candidate farthest from every point so far, while one keeps
/// the spacing. Ids no place is left for go ([`refill`]): spacing before count.
fn training(ffa: &Map, table: Spawns, plot: &mut File) -> Result<Spawns, String> {
    let (candidates, on_islands) = candidates(ffa, Some(plot))?;
    let slots: Vec<Option<(Candidate, String)>> = table
        .into_iter()
        .enumerate()
        .map(|(id, row)| match fit(ffa, &row.0) {
            Ok(()) => Some(row),
            Err(why) => {
                eprintln!("{TRAINING_FFA}: point {id} {why}: its id moves");
                None
            }
        })
        .collect();
    let kept: Vec<Vec3> = slots.iter().flatten().map(|(c, _)| c.position).collect();
    for (j, a) in kept.iter().enumerate() {
        if kept[..j].iter().any(|b| training_distance(*a, *b) < MIN_SPACING) {
            return Err(format!("{TRAINING_FFA}: the table's points are under {MIN_SPACING} m apart"));
        }
    }
    let free = slots.len() - kept.len();
    let pool: Vec<Candidate> = candidates.into_iter().chain(on_islands).collect();
    let placed = extend(&pool, &kept, MIN_SPACING, free, training_distance);
    eprintln!(
        "{TRAINING_FFA}: {} points keep their place, {} of {free} ids take a new one",
        kept.len(),
        placed.len()
    );
    let fresh = placed.into_iter().map(|i| (pool[i], moved_note(pool[i].position)));
    let (points, moved_down) = refill(slots, fresh);
    for (id, from) in moved_down {
        eprintln!("{TRAINING_FFA}: no place is left for id {id}: point {from} takes it, where it stands");
    }
    Ok(points)
}

fn run() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let (mut layers, mut projects, mut plot_dir, mut table) = (Vec::new(), Vec::new(), None, None);
    while let Some(flag) = args.next() {
        let value = args.next().ok_or_else(|| format!("{flag} needs a value"))?;
        match flag.as_str() {
            "--db" | "--patch" => layers.push(PathBuf::from(value)),
            "--project" => projects.push(value),
            "--plots" => plot_dir = Some(PathBuf::from(value)),
            "--points" => table = Some(PathBuf::from(value)),
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
        let mut plot = File::create(plots.join(project.replace('/', "_") + ".csv"))
            .map_err(|e| e.to_string())?;
        // Training Camp's points are Free for All's own project's: kept or
        // placed on it, and checked there.
        let on = if project == TRAINING { TRAINING_FFA } else { &project };
        let (bounds, _) = area(on)?;
        let map = Map::load(&mut resources, &materials, on, bounds)?;
        let points = if project == TRAINING {
            let table = table.as_deref().ok_or("--points is required for Training Camp")?;
            training(&map, read_table(table)?, &mut plot)?
        } else {
            generate(&map, &mut plot)?
        };
        for (i, (c, note)) in points.iter().enumerate() {
            fit(&map, c).map_err(|why| format!("{project}: point {i} {why}"))?;
            let p = c.position;
            println!(
                "{project}\tported_{i:02}\t{}\t{}\t{}\t0\t{}\t0\t30\t2\tkept\t\t{REASON}{note}",
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
    fn empty_ids_take_the_new_places_and_then_the_last_points() {
        // Every empty id gets a new place, lowest id first.
        let slots = vec![Some('a'), None, Some('c'), None];
        assert_eq!(refill(slots.clone(), ['x', 'y', 'z']), (vec!['a', 'x', 'c', 'y'], vec![]));
        // One place for two ids: the last point moves down into the id left.
        let slots = vec![Some('a'), None, None, Some('d'), Some('e')];
        assert_eq!(refill(slots, ['x']), (vec!['a', 'x', 'e', 'd'], vec![(2, 4)]));
        // No place at all: empty ids at the end just go, the others are taken from the end.
        let slots = vec![None, Some('b'), Some('c'), None, Some('e'), None];
        assert_eq!(refill(slots, []), (vec!['e', 'b', 'c'], vec![(0, 4)]));
        assert_eq!(refill(vec![None::<char>, None], []), (vec![], vec![]));
        assert_eq!(refill(vec![Some('a')], ['x']), (vec!['a'], vec![]));
    }
    #[test]
    fn a_new_place_says_where_it_went() {
        assert!(moved_note(Vec3::new(51.0, -0.75, 18.0)).ends_with("fence, spacing in 3D from islands"));
        assert!(moved_note(Vec3::new(0.0, 23.0, 3.0)).contains("raised island west"));
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
    fn reviewed_bounds_include_the_warehouses_west_and_training_camp_is_its_own_project() {
        let (warehouse, clip) = area("v010b_cs_mp_military_1/day_4").unwrap();
        assert!(warehouse[0] < -120.0 && warehouse[2] > 110.0);
        assert!(clip.is_none());
        assert!(area(TRAINING_FFA).is_ok() && area(TRAINING).is_err());
        assert!(area("level_03/day_4").is_err());
    }
}
