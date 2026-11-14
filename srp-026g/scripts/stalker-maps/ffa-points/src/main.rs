//! Author FFA points on the STALKER maps. Explicit TSV output is reviewed
//! and committed, then ../make_ffa_edits.py creates the source edit list.
//! Candidates come from reachable standing floors, pass the same collision
//! checks as imported points, stay clear of harmful zones, and have a six-metre
//! walkable facing. Greedy farthest-point selection distributes 24 points.
//! Training Camp's points are not planned again: those of the committed table
//! (--points) that are still fit on Free for All's own project keep their
//! place and id, but for the ground's, of which two stay. The others' ids take
//! new places on its raised floors (training-islands.tsv, training-towers.tsv:
//! islands, the towers' stairs, cabins) that have a way down on foot, spaced
//! in 3D inside its base's fence (training-fence.tsv).
mod collision;
mod arena;
use collision::{Map, Verdict};
use mapdata::{LinkKind, Materials, Resources, Vec3};
use std::{
    collections::BTreeSet,
    fs::File,
    io::Write,
    path::{Path, PathBuf},
    process::ExitCode,
};

const COUNT: usize = 24;
const HAZARD_MARGIN: f32 = 2.0;
const FACING_REACH: f32 = 6.0;
/// Training Camp as the table and the edit generator name it: its Team
/// Deathmatch project, which is never loaded or edited here.
const TRAINING: &str = "v010b_cs_training_camp/day_4";
/// Free for All's own copy of Training Camp (ffa-projects-1.json): no ammo
/// crates, the walker wall behind its base's fence, a truck in each gateway.
/// Its points are checked and placed on it, so the layers must give it.
const TRAINING_FFA: &str = "v010b_cs_training_camp/day_8";
/// The raised floors are small: they are sampled finer than the 3 m lattice.
const RAISED_LATTICE: f32 = 0.75;
/// A new Training Camp place and its facing's end stay this far inside the fence.
const FENCE_MARGIN: f32 = 2.0;
/// How many of the table's points on the ground stay, the two farthest apart
/// (the owner, 2026-11-14: "make sure all spawns are on islands or on ladders
/// or in rubkas", "some can be on ground, but only a few", "and very rare").
const GROUND_KEPT: usize = 2;
/// The base's ground lies at about y = -0.8: a floor under this height is it.
const GROUND_BELOW: f32 = 1.0;
/// What a row's reason is: the checks every point passes, and how it was spread
/// (no minimum distance: the owner, 2026-11-14, of the spacing, "we don't need
/// this rules at all"; the tables were made 12 m apart, Training Camp's 8 m in 3D).
const REASON: &str = "reachable standing floor; body clear; harmful zones >=2m; walkable facing 6m; spread farthest-first";
const TRAINING_REASON: &str = "reachable standing floor; body clear; harmful zones >=2m; walkable facing 6m; spread farthest-first in 3D";

/// Bounds of the reviewed port's minimaps (stalker-maps@6, stalker-maps-2@1:
/// render_minimap.py's play area). The imported projects have a dummy 2 m
/// levelmap; mapdata's native-map fallback truncates Military Warehouses.
/// Training Camp has open scenery beyond its fence.
fn area(project: &str) -> Result<([f32; 4], Option<f32>), String> {
    match project {
        "v010b_cs_mp_bath/day_4" => Ok(([-48.13, -22.87, 34.12, 53.13], None)),
        "v010b_cs_mp_military_1/day_4" => Ok(([-127.47, -71.12, 115.78, 95.38], None)),
        TRAINING_FFA => Ok(([-47.19, -83.47, 82.81, 91.53], None)),
        "v010b_cs_mp_darkvalley/day_4" => Ok(([212.5, -248.29, 344.0, -166.79], None)),
        "v010b_cs_mp_atp/day_4" => Ok(([90.45, -83.35, 180.2, 29.4], None)),
        "v010b_cs_mp_firestation/day_4" => Ok(([-67.48, -31.41, 50.52, 61.09], None)),
        "v010b_cs_mp_sport_center/day_4" => Ok(([-100.41, -60.72, -16.91, -12.22], None)),
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

/// `count` of `points`, spread: the one nearest the middle, then each the
/// farthest from all chosen, never two at one place; no minimum distance.
fn choose(points: &[Candidate], count: usize) -> Result<Vec<usize>, String> {
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
        if distance[next] <= 0.0 {
            return Err(format!("only {} distinct places for {count} points", chosen.len()));
        }
        chosen.push(next);
    }
    Ok(chosen)
}

/// The distance between two Training Camp points: in 3D.
fn apart(a: Vec3, b: Vec3) -> f32 {
    (a - b).length()
}

/// Up to `limit` more points after `chosen`: the candidate farthest from every
/// point so far, never at a point's own place; no minimum distance.
fn extend(
    points: &[Candidate],
    chosen: &[Vec3],
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
            .filter(|&next| distance[next] > 0.0)
    {
        added.push(next);
        let at = points[next].position;
        for (i, p) in points.iter().enumerate() {
            distance[i] = distance[i].min(distance_of(p.position, at));
        }
    }
    added
}

/// `count` of `points` spread over the ground plane, in ascending order: from
/// each start, the farthest from those taken so far, again and again; the
/// start whose choice keeps the largest smallest gap wins (the first on a tie).
fn spread(points: &[Candidate], count: usize) -> Vec<usize> {
    let Some(more) = count.min(points.len()).checked_sub(1) else {
        return Vec::new();
    };
    let gap = |chosen: &[usize]| {
        let pairs = chosen.iter().enumerate().flat_map(|(j, &a)| chosen[..j].iter().map(move |&b| (a, b)));
        pairs
            .map(|(a, b)| points[a].position.distance_xz(points[b].position))
            .fold(f32::INFINITY, f32::min)
    };
    let from = |start: usize| {
        let mut chosen = vec![start];
        // A point at the start's own place is never the farthest from it.
        let taken = [points[start].position];
        chosen.extend(extend(points, &taken, more, Vec3::distance_xz));
        chosen
    };
    let mut best = (0..points.len())
        .map(from)
        .reduce(|best, next| if gap(&next) > gap(&best) { next } else { best })
        .unwrap_or_default();
    best.sort_unstable();
    best
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

/// A map's points in id order, each with its row's reason.
type Spawns = Vec<(Candidate, String)>;

/// The floors a spawn may stand on, in the grid's order: reached from the
/// map's spawns, with a standing body's headroom, in the play area (Training
/// Camp: [`FENCE_MARGIN`] inside its fence). `plot` samples them.
fn standing_floors(map: &Map, plot: &mut File) -> Result<Vec<u32>, String> {
    let seeds: Vec<_> = map
        .data
        .spawns
        .iter()
        .filter_map(|s| map.grid.locate(s.position, 1.0))
        .collect();
    let reachable = map.grid.distances_from(&seeds);
    let mut floors = Vec::new();
    for node in 0..map.grid.len() as u32 {
        if !reachable.reaches(node) || !map.grid.can_stand(node) {
            continue;
        }
        let p = map.grid.position(node);
        if !in_play_area(map, p, FENCE_MARGIN) {
            continue;
        }
        if node % 16 == 0 {
            writeln!(plot, "{},{},{}", p.x, p.y, p.z).map_err(|e| e.to_string())?;
        }
        floors.push(node);
    }
    Ok(floors)
}

/// Towards a map's middle: where a facing looks on a tie.
fn middle(map: &Map) -> Result<Vec3, String> {
    let (area, _) = area(&map.data.project)?;
    Ok(Vec3::new((area[0] + area[2]) * 0.5, 0.0, (area[1] + area[3]) * 0.5))
}

fn generate(map: &Map, plot: &mut File) -> Result<Spawns, String> {
    let center = middle(map)?;
    let mut cells = BTreeSet::new();
    // A 3 m lattice is dense enough to find cover without doing expensive
    // collision queries for every quarter-metre grid node. Keep floors apart.
    let candidates: Vec<Candidate> = standing_floors(map, plot)?
        .into_iter()
        .filter(|&node| cells.insert(cell(map.grid.position(node), 3.0)))
        .filter_map(|node| candidate(map, node, center))
        .collect();
    eprintln!(
        "{}: {} safe candidates on {} floors",
        map.data.project,
        candidates.len(),
        map.grid.len()
    );
    Ok(choose(&candidates, COUNT)?
        .into_iter()
        .map(|i| (candidates[i], REASON.to_owned()))
        .collect())
}

/// Training Camp's rows of the committed table, in id order: where each
/// point stands and faces.
fn read_table(path: &Path) -> Result<Vec<Candidate>, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    text.lines()
        .filter(|line| line.split('\t').next() == Some(TRAINING))
        .enumerate()
        .map(|(id, line)| {
            let cells: Vec<&str> = line.split('\t').collect();
            let [_, point, x, y, z, _, yaw, _, _, _, _, _, _] = cells[..] else {
                return Err(format!("{TRAINING} row {id}: 13 cells expected"));
            };
            let number = |cell: &str| {
                cell.parse::<f32>()
                    .map_err(|e| format!("{TRAINING} row {id}: {cell}: {e}"))
            };
            if point != format!("ported_{id:02}") {
                return Err(format!("{TRAINING} row {id} is {point}: rows are in id order"));
            }
            let position = Vec3::new(number(x)?, number(y)?, number(z)?);
            Ok(Candidate { position, yaw: number(yaw)? })
        })
        .collect()
}

/// What a Training Camp point stands on.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Floor {
    /// The base's ground.
    Ground,
    /// A raised floor a spawn may stand on, with a way down to the ground on
    /// foot ([`Descent`]).
    Raised(arena::Raised),
}

impl std::fmt::Display for Floor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Ground => f.write_str("ground inside the fence"),
            Self::Raised(raised) => write!(f, "{raised}, way down on foot"),
        }
    }
}

/// The ways down to Training Camp's ground: from which floors a body gets to
/// it by the walk grid's moves (walking, crouching, a jump up a ledge, a drop
/// of the harmless height at most).
struct Descent(mapdata::DistanceMap);

impl Descent {
    fn of(map: &Map) -> Self {
        let ground: Vec<u32> = (0..map.grid.len() as u32)
            .filter(|&node| {
                let p = map.grid.position(node);
                p.y < GROUND_BELOW && in_play_area(map, p, 0.0)
            })
            .collect();
        Self(map.grid.distances_to(&ground))
    }

    /// What the floor at `node`, at `p`, is for a spawn: the ground, a
    /// raised floor it may stand on, or neither.
    fn floor(&self, node: u32, p: Vec3) -> Result<Floor, String> {
        match arena::raised_at(p.x, p.y, p.z) {
            Some(raised) if self.0.reaches(node) => Ok(Floor::Raised(raised)),
            Some(raised) => Err(format!("has no way down from {raised}")),
            None if p.y < GROUND_BELOW => Ok(Floor::Ground),
            None => Err("stands on no island, stairs or cabin".to_owned()),
        }
    }

    /// The way down from the floor at `node`, for the log: its length, its
    /// drops and jumps, and the raised floors it passes.
    fn way(&self, grid: &mapdata::WalkGrid, mut node: u32) -> String {
        let (mut metres, mut drops, mut jumps, mut highest) = (0.0, 0, 0, 0.0f32);
        let mut passes: Vec<String> = Vec::new();
        while let Some((next, kind)) = self.0.next_step(grid, node) {
            let (from, to) = (grid.position(node), grid.position(next));
            metres += (to - from).length();
            match kind {
                LinkKind::Walk => {}
                LinkKind::Drop => (drops, highest) = (drops + 1, highest.max(from.y - to.y)),
                LinkKind::Jump => jumps += 1,
            }
            let on = match arena::raised_at(to.x, to.y, to.z) {
                Some(raised) => raised.to_string(),
                None if to.y < GROUND_BELOW => "the ground".to_owned(),
                None => "other floors".to_owned(),
            };
            if passes.last() != Some(&on) {
                passes.push(on);
            }
            node = next;
        }
        format!(
            "{metres:.0} m, {drops} drops (the highest {highest:.1} m), {jumps} jumps, over {}",
            passes.join(" > ")
        )
    }
}

/// Training Camp on Free for All's own project as the layers give it. The
/// table's points on a raised floor that are still fit keep their place and
/// id, and so do [`GROUND_KEPT`] of those on the ground, the ones farthest
/// apart. The others' ids take new places on the raised floors (islands, the
/// towers' stairs, cabins) that have a way down: the candidate farthest from
/// every point so far (in 3D, no minimum distance). Ids no place is left for
/// go ([`refill`]).
fn training(ffa: &Map, table: Vec<Candidate>, plot: &mut File) -> Result<Spawns, String> {
    let descent = Descent::of(ffa);
    let rows: Vec<Option<(Candidate, Floor)>> = table
        .into_iter()
        .enumerate()
        .map(|(id, c)| {
            let floor = fit(ffa, &c).and_then(|()| {
                let node = ffa.grid.locate(c.position, 0.6).ok_or("lost its floor")?;
                descent.floor(node, c.position)
            });
            match floor {
                Ok(floor) => Some((c, floor)),
                Err(why) => {
                    eprintln!("{TRAINING_FFA}: point {id} {why}: its id moves");
                    None
                }
            }
        })
        .collect();
    let (ids, ground): (Vec<usize>, Vec<Candidate>) = rows
        .iter()
        .enumerate()
        .filter_map(|(id, row)| row.filter(|(_, floor)| *floor == Floor::Ground).map(|(c, _)| (id, c)))
        .unzip();
    let stay: Vec<usize> = spread(&ground, GROUND_KEPT).into_iter().map(|i| ids[i]).collect();
    eprintln!(
        "{TRAINING_FFA}: {} points on the ground, of which {stay:?} stay there",
        ids.len()
    );
    let slots: Vec<Option<(Candidate, Floor)>> = rows
        .into_iter()
        .enumerate()
        .map(|(id, row)| row.filter(|(_, floor)| *floor != Floor::Ground || stay.contains(&id)))
        .collect();
    let kept: Vec<Vec3> = slots.iter().flatten().map(|(c, _)| c.position).collect();
    let center = middle(ffa)?;
    let mut cells = BTreeSet::new();
    let pool: Vec<(Candidate, Floor)> = standing_floors(ffa, plot)?
        .into_iter()
        .filter_map(|node| {
            let p = ffa.grid.position(node);
            let floor = descent.floor(node, p).ok().filter(|floor| *floor != Floor::Ground)?;
            cells.insert(cell(p, RAISED_LATTICE)).then_some(())?;
            Some((candidate(ffa, node, center)?, floor))
        })
        .collect();
    let free = slots.len() - kept.len();
    // The cabins first: they are the smallest of the raised floors, and the
    // farthest place from every point is never in one.
    let (cabins, open): (Vec<_>, Vec<_>) = pool
        .iter()
        .copied()
        .partition(|(_, floor)| matches!(floor, Floor::Raised(arena::Raised::Cabin(_))));
    let (mut taken, mut fresh) = (kept.clone(), Vec::new());
    for places in [cabins, open] {
        let candidates: Vec<Candidate> = places.iter().map(|(c, _)| *c).collect();
        for i in extend(&candidates, &taken, free - fresh.len(), apart) {
            taken.push(candidates[i].position);
            fresh.push(places[i]);
        }
    }
    eprintln!(
        "{TRAINING_FFA}: {} safe candidates on raised floors with a way down; {} points keep their place, {} of {free} ids take a new one",
        pool.len(),
        kept.len(),
        fresh.len()
    );
    if std::env::var_os("FFA_POINTS_CANDIDATES").is_some() {
        for (c, floor) in &pool {
            eprintln!("candidate\t{}\t{}\t{}\t{}\t{floor}", c.position.x, c.position.y, c.position.z, c.yaw);
        }
    }
    let (points, moved_down) = refill(slots, fresh);
    for (id, from) in moved_down {
        eprintln!("{TRAINING_FFA}: no place is left for id {id}: point {from} takes it, where it stands");
    }
    for (id, (c, floor)) in points.iter().enumerate() {
        if let (Floor::Raised(raised), Some(node)) = (floor, ffa.grid.locate(c.position, 0.6)) {
            eprintln!("{TRAINING_FFA}: point {id} on {raised}, down {}", descent.way(&ffa.grid, node));
        }
    }
    Ok(points
        .into_iter()
        .map(|(c, floor)| (c, format!("{TRAINING_REASON}; {floor}")))
        .collect())
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
        for (i, (c, reason)) in points.iter().enumerate() {
            fit(&map, c).map_err(|why| format!("{project}: point {i} {why}"))?;
            let p = c.position;
            println!(
                "{project}\tported_{i:02}\t{}\t{}\t{}\t0\t{}\t0\t30\t2\tkept\t\t{reason}",
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
    fn island_points_go_farthest_first_never_at_a_taken_place() {
        // A 20 m strip at y 20, one point a metre: its 0 m end taken, the 20 m
        // end first, then the middle, then every other place but the taken one.
        let points: Vec<_> = (0..=20)
            .map(|x| Candidate { position: Vec3::new(x as f32, 20.0, 0.0), yaw: 0.0 })
            .collect();
        let d3 = |a: Vec3, b: Vec3| (a - b).length();
        let all = usize::MAX;
        let after_end = extend(&points, &[Vec3::new(0.0, 20.0, 0.0)], all, d3);
        assert_eq!((after_end.len(), &after_end[..2]), (20, &[20, 10][..]));
        assert!(!after_end.contains(&0));
        assert_eq!(extend(&points, &[], all, d3).len(), 21);
        assert_eq!(extend(&points, &[], 1, d3), vec![0]);
        // A spawn 20 m below the strip's middle: in 3D the middle is 20 m from
        // it and comes third; in plan it is on it, and 5 m does.
        let below = [Vec3::new(10.0, 0.0, 0.0)];
        assert_eq!(extend(&points, &below, 3, d3), vec![0, 20, 10]);
        assert_eq!(extend(&points, &below, 3, Vec3::distance_xz), vec![0, 20, 5]);
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
    fn the_ground_points_that_stay_are_the_farthest_apart() {
        let at = |x: f32, z: f32| Candidate { position: Vec3::new(x, 0.0, z), yaw: 0.0 };
        // A 30 x 40 m rectangle's corners and its middle: the diagonal, and the first one on a tie.
        let points = [at(0.0, 0.0), at(30.0, 0.0), at(15.0, 20.0), at(0.0, 40.0), at(30.0, 40.0)];
        assert_eq!(spread(&points, 2), vec![0, 4]);
        // Three: that diagonal and a third corner, 30 m apart at the least (the first on a tie).
        assert_eq!(spread(&points, 3), vec![0, 1, 4]);
        assert_eq!(spread(&points, 1), vec![0]);
        assert_eq!(spread(&points, 9), vec![0, 1, 2, 3, 4]);
        assert!(spread(&points, 0).is_empty() && spread(&[], 2).is_empty());
    }
    #[test]
    fn a_row_says_what_its_point_stands_on() {
        assert_eq!(Floor::Ground.to_string(), "ground inside the fence");
        let on = |raised| Floor::Raised(raised).to_string();
        assert_eq!(on(arena::Raised::Island("west")), "raised island west, way down on foot");
        assert_eq!(on(arena::Raised::Stairs("east")), "stairs of tower east, way down on foot");
        assert_eq!(on(arena::Raised::Cabin("east_upper")), "cabin east_upper, way down on foot");
    }
    #[test]
    fn only_too_few_places_are_refused_points_may_stand_close() {
        assert!(choose(&[], 24).is_err());
        let at = |x: f32| Candidate { position: Vec3::new(x, 0.0, 0.0), yaw: 0.0 };
        assert!(choose(&[at(0.0), at(0.0)], 2).is_err());
        assert_eq!(choose(&[at(0.0), at(1.0)], 2).unwrap().len(), 2);
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
