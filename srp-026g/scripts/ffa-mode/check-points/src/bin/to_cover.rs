//! A map's Free for All spawns by the spawn rule (the owner, 2026-11-14:
//! "there should be a rule that spawns are inside buildings or close to two
//! walls or some cover", "and none at the perimeter", "let's make other maps
//! have x2 spawns too"): the rule is `ffa_check_points::cover` (cover, past
//! the perimeter band); the spawns that fail it get new places that pass it,
//! and new points that pass it make the count `--times` what it was.
//!
//! A solo spawn stays where it is when it passes the spawn check
//! ([`Map::check`]: standing floor, body clear), harmful zones at least
//! HAZARD_MARGIN away and the rule ([`Placement::holds`]), however near
//! another spawn it stands (the owner, 2026-11-14: "spawns can be as close as
//! possible, the only thing that matters is whether people can spawn close to
//! each other": the respawn's 20 m enemy exclusion keeps players apart). Its
//! facing: its own when its FACING_REACH ahead is walkable and it sees VIEW
//! ahead (not into a wall), else the facing a new spawn gets (it is turned;
//! its place stays), else its own.
//!
//! The others, and the new points, take new places: the reachable standing
//! floors past the band, one in each cell of 2 m and 2 m of height, that pass
//! the same checks and the rule, with a facing: of the 32 headings whose
//! FACING_REACH ahead is walkable (no harmful zone at its end), the one that
//! sees farthest (to SIGHT), into the area on a tie, seeing VIEW at least.
//! They are chosen one at a time, each the place that brings the area's inner
//! cells (past the band) nearest to a spawn in all (the sum of each cell's
//! distance to its nearest spawn falls the most), so the inner area has
//! places to spawn everywhere: a preference, no distance kept. The replaced
//! spawns' ids go to the new places, the nearest pair first; the places left
//! are new points, numbered after the project's highest point id. A map with
//! fewer candidates than its failing spawns is refused; one that cannot hold
//! the count takes what it holds and says so.
//!
//! The walk grid is seeded by the spawns and its area holds them all, so a
//! map whose spawns moved is measured again with them where they stand, and
//! changed again, until every spawn stays as it is and none is added (at most
//! ROUNDS rounds): its spawns pass the rule on the project the list makes.
//!
//! - `--below <project>=<height>`: no spawn of that project stands above the
//!   height (London 14.5: none on its rotunda's third storey, floor 17.92;
//!   the owner, 2026-11-13: "london shouldn't have spawns on the third floor");
//! - `--fence <project>=<tsv>`: its area, its band and its new places are
//!   inside that polygon (`x`, `z` columns): Training Camp's fence;
//! - `--hold <project>`: its current spawns all stay as they are, whatever
//!   the rule makes of them (Training Camp: the owner's arena); only new
//!   points are added (and `--below` is theirs alone).
//!
//! Prints the changed and new spawns as checked.tsv rows (`point` the point
//! id; `verdict` `replaced`, `turned` or `added`): cover-maps.tsv, made into
//! an edit list by make_cover_edits.py; writes <plots>/<project>.json (the
//! area, every spawn before with its verdict, the set after) for a review
//! render.
//!
//! ```text
//! to_cover --db <resources.db> --patch <resources.db.patch_12> ... --plots <folder> \
//!     --times 2 --project level_02/day_8 ... --project v010b_cs_mp_bath/day_8@x0,z0,x1,z1 \
//!     --below level_09/day_8=14.5 --project v010b_cs_training_camp/day_8@x0,z0,x1,z1 \
//!     --fence v010b_cs_training_camp/day_8=<training-fence.tsv> \
//!     --hold v010b_cs_training_camp/day_8 --below v010b_cs_training_camp/day_8=1
//! ```
use ffa_check_points::cover::{Area, Placement, Sight};
use ffa_check_points::{Map, Verdict};
use mapdata::{MapData, Materials, Resources, Spawn, Vec3};
use std::{collections::BTreeSet, fmt::Write as _, path::PathBuf, process::ExitCode};

const HAZARD_MARGIN: f32 = 2.0;
const FACING_REACH: f32 = 6.0;
/// How far ahead a facing looks, and what it must see at least.
const SIGHT: f32 = 20.0;
const VIEW: f32 = 8.0;
/// Candidates: one in each cell of this many metres in plan and in height.
const SAMPLE: f32 = 2.0;

/// A current solo spawn, by what the rule makes of it.
enum Before {
    /// It stays: with its own facing, or turned to `yaw` (`turned`).
    Kept { turned: Option<f32> },
    /// It fails: why.
    Fails(String),
    /// It fails (why), and stays as it is: a map given with `--hold`.
    Held(String),
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

fn inside(bounds: Option<[f32; 4]>, p: Vec3) -> bool {
    bounds.is_none_or(|b| p.x >= b[0] && p.z >= b[1] && p.x <= b[2] && p.z <= b[3])
}

/// The heading a spawn of these Euler angles faces (some donor points are
/// turned by pi about x and z: their y alone is not their heading).
fn heading_of(rotation: Vec3) -> f32 {
    mapdata::math::heading(mapdata::math::forward(rotation))
}

/// What the eyes see along `yaw` from the floor `node`, when its
/// FACING_REACH ahead is walkable: metres, to SIGHT.
fn sight_along(map: &Map, bounds: Option<[f32; 4]>, node: u32, yaw: f32) -> Option<f32> {
    let p = map.grid.position(node);
    let dir = mapdata::math::forward(Vec3::new(0.0, yaw, 0.0));
    let end = map.grid.locate(p + dir * FACING_REACH, 0.4)?;
    let at = map.grid.position(end);
    if !map.grid.can_stand(end)
        || (at.y - p.y).abs() > 0.6
        || harmful(map, at)
        || !inside(bounds, at)
        || !map
            .grid
            .walks_straight(node, end, FACING_REACH + 0.5, |n| map.grid.can_stand(n))
    {
        return None;
    }
    let eye = p + Vec3::new(0.0, 1.6, 0.0);
    Some(map.rays.ray_cast(eye, dir, SIGHT).unwrap_or(SIGHT))
}

/// The facing a spawn at `node` gets: the walkable heading that sees
/// farthest, into the area (towards `centre`) on a tie; seeing VIEW at least.
fn facing(map: &Map, bounds: Option<[f32; 4]>, node: u32, centre: Vec3) -> Option<f32> {
    let toward = centre - map.grid.position(node);
    (0..32)
        .filter_map(|i| {
            let dir = mapdata::math::forward(Vec3::new(
                0.0,
                i as f32 * std::f32::consts::TAU / 32.0,
                0.0,
            ));
            let yaw = mapdata::math::heading(dir);
            let sight = sight_along(map, bounds, node, yaw)?;
            Some((yaw, sight, dir.x * toward.x + dir.z * toward.z))
        })
        .filter(|f| f.1 >= VIEW)
        .max_by(|a, b| a.1.total_cmp(&b.1).then(a.2.total_cmp(&b.2)))
        .map(|(yaw, _, _)| yaw)
}

/// The rule and the spawn check at `p`: its ground (the point put on it), or
/// why not.
fn checked(map: &Map, sight: &Sight, area: &Area, p: Vec3) -> Result<(Vec3, Placement), String> {
    let feet = match map.check(p) {
        Verdict::Kept => p,
        Verdict::Nudged { ground } => Vec3::new(p.x, ground, p.z),
        Verdict::Dropped(why) => return Err(why),
    };
    if harmful(map, feet) {
        return Err(format!("a harmful zone within {HAZARD_MARGIN} m"));
    }
    let placement = Placement::of(sight, area, feet);
    if !placement.inside {
        return Err(format!(
            "perimeter band (depth {:.1} m <= {:.1} m)",
            placement.depth,
            area.band()
        ));
    }
    if !placement.cover.holds() {
        return Err("no cover".into());
    }
    Ok((feet, placement))
}

/// The area's inner cells (past the band): their middles.
fn inner_cells(area: &Area) -> Vec<(f32, f32)> {
    (0..area.width * area.height)
        .filter(|&k| area.depth[k].is_some_and(|d| d > area.band()))
        .map(|k| {
            (
                area.x0 + ((k % area.width) as f32 + 0.5) * Area::CELL,
                area.z0 + ((k / area.width) as f32 + 0.5) * Area::CELL,
            )
        })
        .collect()
}

/// The new places, one at a time: of the candidates not chosen yet, the one
/// whose taking lowers the inner cells' summed distance to their nearest
/// spawn (`existing` and those chosen) the most, so the inner area has
/// places to spawn everywhere; until `count` or none is left. No distance is
/// kept: a place near a spawn is taken when nothing farther helps more.
fn choose(
    candidates: &[Candidate],
    existing: &[Vec3],
    cells: &[(f32, f32)],
    count: usize,
) -> Vec<usize> {
    let plan = |c: (f32, f32), p: Vec3| (c.0 - p.x).hypot(c.1 - p.z);
    let mut nearest: Vec<f32> = cells
        .iter()
        .map(|&c| {
            existing
                .iter()
                .map(|&p| plan(c, p))
                .fold(f32::INFINITY, f32::min)
        })
        .collect();
    // With no spawn yet, a cell's distance counts as the area's size.
    let (lo, hi) = cells.iter().fold(
        (
            (f32::INFINITY, f32::INFINITY),
            (f32::NEG_INFINITY, f32::NEG_INFINITY),
        ),
        |(lo, hi), &c| {
            (
                (lo.0.min(c.0), lo.1.min(c.1)),
                (hi.0.max(c.0), hi.1.max(c.1)),
            )
        },
    );
    let far = (hi.0 - lo.0).hypot(hi.1 - lo.1).max(1.0);
    for d in &mut nearest {
        *d = d.min(far);
    }
    let mut chosen: Vec<usize> = Vec::new();
    while chosen.len() < count {
        let best = (0..candidates.len())
            .filter(|i| !chosen.contains(i))
            .map(|i| {
                let p = candidates[i].position;
                let gain: f64 = cells
                    .iter()
                    .zip(&nearest)
                    .map(|(&c, &d)| f64::from((d - plan(c, p)).max(0.0)))
                    .sum();
                (i, gain)
            })
            .max_by(|a, b| a.1.total_cmp(&b.1).then(b.0.cmp(&a.0)));
        let Some((next, _)) = best else { break };
        chosen.push(next);
        let p = candidates[next].position;
        for (d, &c) in nearest.iter_mut().zip(cells) {
            *d = d.min(plan(c, p));
        }
    }
    chosen
}

/// The replaced spawns' ids for the new places: the nearest (old place, new
/// place) pair first; the places left over have none (they are new points).
fn assign(old: &[(i32, Vec3)], new: &[Vec3]) -> Vec<Option<i32>> {
    let mut pairs: Vec<(f32, usize, usize)> = old
        .iter()
        .enumerate()
        .flat_map(|(i, (_, a))| {
            new.iter()
                .enumerate()
                .map(move |(j, b)| (a.distance_xz(*b), i, j))
        })
        .collect();
    pairs.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)).then(a.2.cmp(&b.2)));
    let (mut id, mut used) = (vec![None; new.len()], vec![false; old.len()]);
    for (_, i, j) in pairs {
        if !used[i] && id[j].is_none() {
            used[i] = true;
            id[j] = Some(old[i].0);
        }
    }
    id
}

/// Whether (x, z) is inside the polygon `vertices` (in order).
fn within(vertices: &[[f32; 2]], x: f32, z: f32) -> bool {
    let mut inside = false;
    for (a, b) in vertices
        .iter()
        .zip(vertices.iter().cycle().skip(1))
        .take(vertices.len())
    {
        if (a[1] > z) != (b[1] > z) && x < a[0] + (z - a[1]) * (b[0] - a[0]) / (b[1] - a[1]) {
            inside = !inside;
        }
    }
    inside
}

/// A polygon's TSV (`x`, `z` columns after a header line).
fn polygon(path: &str) -> Result<Vec<[f32; 2]>, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;
    let vertices = text
        .lines()
        .skip(1)
        .filter(|l| !l.trim().is_empty())
        .map(|line| {
            let (x, z) = line
                .split_once('\t')
                .ok_or_else(|| format!("{path}: {line}"))?;
            Ok([
                x.parse().map_err(|_| format!("{path}: {line}"))?,
                z.trim().parse().map_err(|_| format!("{path}: {line}"))?,
            ])
        })
        .collect::<Result<Vec<_>, String>>()?;
    if vertices.len() < 3 {
        return Err(format!("{path}: not a polygon"));
    }
    Ok(vertices)
}

struct Args {
    layers: Vec<PathBuf>,
    projects: Vec<(String, Option<[f32; 4]>)>,
    /// (project, the highest a spawn may stand there).
    below: Vec<(String, f32)>,
    /// (project, the fence its area and new places are inside).
    fences: Vec<(String, Vec<[f32; 2]>)>,
    /// Projects whose current spawns all stay as they are.
    hold: Vec<String>,
    /// A map's spawns after: this many times its current ones.
    times: usize,
    plots: PathBuf,
}

fn parse() -> Result<Args, String> {
    let mut args = std::env::args().skip(1);
    let (mut layers, mut projects, mut below, mut fences, mut hold, mut plots) = (
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
        None,
    );
    let mut times = 1;
    let named = |value: &str, what: &str| -> Result<(String, String), String> {
        let (p, v) = value
            .split_once('=')
            .ok_or_else(|| format!("{value}: <project>={what}"))?;
        Ok((p.to_owned(), v.to_owned()))
    };
    while let Some(flag) = args.next() {
        let value = args.next().ok_or_else(|| format!("{flag} needs a value"))?;
        match flag.as_str() {
            "--db" | "--patch" => layers.push(PathBuf::from(value)),
            "--project" => projects.push(match value.split_once('@') {
                None => (value, None),
                Some((p, b)) => {
                    let v: Vec<f32> = b
                        .split(',')
                        .map(|s| s.parse().map_err(|_| format!("bad bounds {b}")))
                        .collect::<Result<_, _>>()?;
                    let b: [f32; 4] = v.try_into().map_err(|_| format!("bad bounds {b}"))?;
                    (p.to_owned(), Some(b))
                }
            }),
            "--below" => {
                let (p, y) = named(&value, "<height>")?;
                below.push((p, y.parse().map_err(|_| format!("bad height {y}"))?));
            }
            "--fence" => {
                let (p, file) = named(&value, "<tsv>")?;
                fences.push((p, polygon(&file)?));
            }
            "--hold" => hold.push(value),
            "--times" => times = value.parse().map_err(|_| format!("bad --times {value}"))?,
            "--plots" => plots = Some(PathBuf::from(value)),
            _ => return Err(format!("unknown flag {flag}")),
        }
    }
    if layers.is_empty() || projects.is_empty() {
        return Err("--db and at least one --project are required".into());
    }
    let known = |p: &String| projects.iter().any(|(q, _)| q == p);
    if let Some(p) = below
        .iter()
        .map(|(p, _)| p)
        .chain(fences.iter().map(|(p, _)| p))
        .chain(&hold)
        .find(|p| !known(p))
    {
        return Err(format!("{p}: not a --project"));
    }
    if times == 0 {
        return Err("--times is at least 1".into());
    }
    Ok(Args {
        layers,
        projects,
        below,
        fences,
        hold,
        times,
        plots: plots.ok_or("--plots is required")?,
    })
}

/// A project's settings.
struct Setting<'a> {
    bounds: Option<[f32; 4]>,
    /// The highest a spawn may stand.
    top: Option<f32>,
    /// The fence its area and new places are inside.
    fence: Option<&'a [[f32; 2]]>,
    /// Whether its current spawns all stay as they are.
    hold: bool,
    /// How many solo spawns it has after.
    total: usize,
}

/// One round of the rule on a map's solo spawns as they stand.
struct Round {
    area: Area,
    /// The solo spawns, in point id order, and what the rule makes of each.
    before: Vec<(Spawn, Before)>,
    /// The failing ones' new places, by point id.
    moved: Vec<(i32, Candidate)>,
    /// New points up to the total, by their new point ids.
    added: Vec<(i32, Candidate)>,
    candidates: usize,
}

impl Round {
    fn of(map: &Map, sight: &Sight, setting: &Setting) -> Result<Self, String> {
        let Setting {
            bounds,
            top,
            fence,
            hold,
            total,
        } = *setting;
        let floors: Vec<u32> = map
            .standing_floors(bounds)
            .into_iter()
            .filter(|&n| {
                let p = map.grid.position(n);
                fence.is_none_or(|f| within(f, p.x, p.z))
            })
            .collect();
        let area = Area::new(
            &floors
                .iter()
                .map(|&n| map.grid.position(n))
                .collect::<Vec<_>>(),
        )?;
        let centre = Vec3::new(area.centroid.0, 0.0, area.centroid.1);
        let mut solo: Vec<Spawn> = map
            .data
            .spawns
            .iter()
            .filter(|s| s.team == 2)
            .cloned()
            .collect();
        solo.sort_by_key(|s| s.point_id);
        let before: Vec<(Spawn, Before)> = solo
            .into_iter()
            .map(|s| {
                let verdict = (|| {
                    if let Some(top) = top.filter(|&top| !hold && s.position.y > top) {
                        return Err(format!("above {top} m"));
                    }
                    let (feet, _) = checked(map, sight, &area, s.position)?;
                    let node = map
                        .grid
                        .locate(feet, 0.6)
                        .ok_or("no walk-grid floor under it")?;
                    let turned = match sight_along(map, bounds, node, heading_of(s.rotation)) {
                        Some(seen) if seen >= VIEW => None,
                        _ => facing(map, bounds, node, centre),
                    };
                    Ok(turned)
                })();
                let verdict = match verdict {
                    Ok(turned) if !hold => Before::Kept { turned },
                    Ok(_) => Before::Kept { turned: None },
                    Err(why) if hold => Before::Held(why),
                    Err(why) => Before::Fails(why),
                };
                (s, verdict)
            })
            .collect();
        let stays: Vec<Vec3> = before
            .iter()
            .filter(|(_, v)| !matches!(v, Before::Fails(_)))
            .map(|(s, _)| s.position)
            .collect();
        let failing: Vec<(i32, Vec3)> = before
            .iter()
            .filter(|(_, v)| matches!(v, Before::Fails(_)))
            .map(|(s, _)| (s.point_id, s.position))
            .collect();
        let mut cells = BTreeSet::new();
        let mut candidates = Vec::new();
        for node in floors {
            let p = map.grid.position(node);
            if !area.inside(p)
                || top.is_some_and(|top| p.y > top)
                || !cells.insert((
                    (p.x / SAMPLE).floor() as i32,
                    (p.y / SAMPLE).floor() as i32,
                    (p.z / SAMPLE).floor() as i32,
                ))
            {
                continue;
            }
            let Ok((feet, _)) = checked(map, sight, &area, p) else {
                continue;
            };
            if let Some(yaw) = facing(map, bounds, node, centre) {
                candidates.push(Candidate {
                    position: feet,
                    yaw,
                });
            }
        }
        let wanted = failing.len() + total.saturating_sub(before.len());
        let places: Vec<Candidate> = choose(&candidates, &stays, &inner_cells(&area), wanted)
            .into_iter()
            .map(|i| candidates[i])
            .collect();
        if places.len() < failing.len() {
            return Err(format!(
                "{} places pass the rule for the {} spawns that fail it",
                places.len(),
                failing.len()
            ));
        }
        let ids = assign(
            &failing,
            &places.iter().map(|c| c.position).collect::<Vec<_>>(),
        );
        let mut next = map
            .data
            .spawns
            .iter()
            .map(|s| s.point_id)
            .max()
            .unwrap_or(-1)
            + 1;
        let (mut moved, mut added) = (Vec::new(), Vec::new());
        for (id, place) in ids.into_iter().zip(places) {
            match id {
                Some(id) => moved.push((id, place)),
                None => {
                    added.push((next, place));
                    next += 1;
                }
            }
        }
        Ok(Self {
            area,
            before,
            moved,
            added,
            candidates: candidates.len(),
        })
    }

    fn turned(&self) -> usize {
        self.before
            .iter()
            .filter(|(_, v)| matches!(v, Before::Kept { turned: Some(_) }))
            .count()
    }

    /// Whether every spawn stays as it is and none is added.
    fn settled(&self) -> bool {
        self.moved.is_empty() && self.added.is_empty() && self.turned() == 0
    }

    /// The map's spawns with this round's changes: a new point is a copy of
    /// the first solo spawn at its place.
    fn apply(&self, spawns: &mut Vec<Spawn>) {
        for s in spawns.iter_mut().filter(|s| s.team == 2) {
            if let Some((_, c)) = self.moved.iter().find(|(id, _)| *id == s.point_id) {
                s.position = c.position;
                s.rotation = Vec3::new(0.0, c.yaw, 0.0);
            }
            let turned = self.before.iter().find_map(|(b, v)| match v {
                Before::Kept { turned: Some(yaw) } if b.point_id == s.point_id => Some(*yaw),
                _ => None,
            });
            if let Some(yaw) = turned {
                s.rotation = Vec3::new(0.0, yaw, 0.0);
            }
        }
        if let Some((model, _)) = self.before.first() {
            for (id, c) in &self.added {
                spawns.push(Spawn {
                    position: c.position,
                    rotation: Vec3::new(0.0, c.yaw, 0.0),
                    point_id: *id,
                    ..model.clone()
                });
            }
        }
    }
}

/// Rounds a map takes at most to settle.
const ROUNDS: usize = 6;

fn run() -> Result<(), String> {
    let args = parse()?;
    std::fs::create_dir_all(&args.plots).map_err(|e| e.to_string())?;
    let mut resources = Resources::open_layers(&args.layers).map_err(|e| e.to_string())?;
    let materials = Materials::load(&mut resources).map_err(|e| e.to_string())?;
    println!(
        "project\tpoint\tx\ty\tz\trx\try\trz\tenemy_check_radius\tinvulnerability_time\tverdict\tground\treason"
    );
    for (project, bounds) in &args.projects {
        eprintln!("loading {project}");
        let mut data =
            MapData::load(&mut resources, &materials, project).map_err(|e| e.to_string())?;
        let setting = Setting {
            bounds: *bounds,
            top: args
                .below
                .iter()
                .find(|(p, _)| p == project)
                .map(|&(_, y)| y),
            fence: args
                .fences
                .iter()
                .find(|(p, _)| p == project)
                .map(|(_, f)| f.as_slice()),
            hold: args.hold.contains(project),
            total: args.times * data.spawns.iter().filter(|s| s.team == 2).count(),
        };
        let map = Map::of(data.clone(), *bounds);
        let sight = Sight::of(&map);
        let first = Round::of(&map, &sight, &setting).map_err(|e| format!("{project}: {e}"))?;
        // The walk grid is seeded by the spawns and its area holds them all:
        // moved and new spawns are measured again, until every one stays.
        let mut later: Option<Round> = None;
        let mut rounds = 1;
        loop {
            let round = later.as_ref().unwrap_or(&first);
            if round.settled() {
                break;
            }
            if rounds == ROUNDS {
                return Err(format!("{project}: not settled in {ROUNDS} rounds"));
            }
            round.apply(&mut data.spawns);
            let map = Map::of(data.clone(), *bounds);
            later = Some(Round::of(&map, &sight, &setting).map_err(|e| format!("{project}: {e}"))?);
            rounds += 1;
        }
        let last = later.as_ref().unwrap_or(&first);
        let area = &last.area;
        // What became of each spawn: moved, turned, as it was, or added.
        let after: Vec<(Spawn, &str)> = last
            .before
            .iter()
            .map(|(now, _)| {
                let state = match first
                    .before
                    .iter()
                    .find(|(s, _)| s.point_id == now.point_id)
                {
                    None => "added",
                    Some((s, _)) if (now.position - s.position).length() > 1e-4 => "new",
                    Some((s, _)) if (now.rotation - s.rotation).length() > 1e-6 => "turned",
                    Some(_) => "kept",
                };
                (now.clone(), state)
            })
            .collect();
        let count = |state: &str| after.iter().filter(|(_, s)| *s == state).count();
        let short = setting.total.saturating_sub(after.len());
        eprintln!(
            "{project}: depth max {:.1} m, band {:.1} m, {} solo -> {}, {} kept ({} turned), {} replaced, {} added, {} candidates; settled in {rounds} rounds{}",
            first.area.max,
            first.area.band(),
            first.before.len(),
            after.len(),
            count("kept") + count("turned"),
            count("turned"),
            count("new"),
            count("added"),
            first.candidates,
            if short > 0 {
                format!("; SHORT of {} by {short}", setting.total)
            } else {
                String::new()
            }
        );
        let reason = |what: &str, p: Vec3| {
            let placement = Placement::of(&sight, area, p);
            format!(
                "{what}; cover {} (depth {:.1} m past the band's {:.1} m); reachable standing floor; body clear; harmful zones >={HAZARD_MARGIN}m; walkable facing {FACING_REACH}m seeing >={VIEW}m{}{}",
                placement.cover.name(),
                placement.depth,
                area.band(),
                setting
                    .top
                    .map_or(String::new(), |top| format!("; at most {top} m high")),
                if setting.fence.is_some() {
                    "; inside the fence"
                } else {
                    ""
                }
            )
        };
        for (s, state) in &after {
            let (verdict, what) = match *state {
                "new" => ("replaced", "new place"),
                "added" => ("added", "new point"),
                "turned" => ("turned", "kept, turned"),
                _ => continue,
            };
            let p = s.position;
            println!(
                "{project}\t{}\t{}\t{}\t{}\t0\t{}\t0\t20\t0\t{verdict}\t\t{}",
                s.point_id,
                p.x,
                p.y,
                p.z,
                s.rotation.y,
                reason(what, p)
            );
        }
        // The plot: the area and the verdicts of the first round, the spawns
        // as they settled.
        let f = |v: f32| format!("{v:.3}");
        let mut json = String::new();
        let a = &first.area;
        write!(
            json,
            "{{\"project\":\"{project}\",\"cell\":{},\"x0\":{},\"z0\":{},\"width\":{},\"height\":{},\"max\":{},\"band\":{},\"candidates\":{},\"rounds\":{rounds},\"total\":{},\"short\":{short},",
            Area::CELL,
            f(a.x0),
            f(a.z0),
            a.width,
            a.height,
            f(a.max),
            f(a.band()),
            first.candidates
            , setting.total
        )
        .unwrap();
        let depth: Vec<String> = a
            .depth
            .iter()
            .map(|d| d.map_or("-1".into(), |d| format!("{d:.1}")))
            .collect();
        write!(json, "\"depth\":[{}],", depth.join(",")).unwrap();
        let point = |area: &Area, s: &Spawn, extra: String| {
            let placement = Placement::of(&sight, area, s.position);
            format!(
                "{{\"id\":{},\"x\":{},\"y\":{},\"z\":{},\"yaw\":{},\"cover\":\"{}\",\"depth\":{},{extra}}}",
                s.point_id,
                f(s.position.x),
                f(s.position.y),
                f(s.position.z),
                f(heading_of(s.rotation)),
                placement.cover.name(),
                f(placement.depth)
            )
        };
        let rows: Vec<String> = first
            .before
            .iter()
            .map(|(s, v)| {
                let extra = match v {
                    Before::Kept { .. } => "\"pass\":true,\"why\":\"\"".to_owned(),
                    Before::Fails(why) | Before::Held(why) => {
                        format!("\"pass\":false,\"why\":\"{}\"", why.replace('"', "'"))
                    }
                };
                point(a, s, extra)
            })
            .collect();
        write!(json, "\"before\":[{}],", rows.join(",")).unwrap();
        let rows: Vec<String> = after
            .iter()
            .map(|(s, state)| point(area, s, format!("\"state\":\"{state}\"")))
            .collect();
        write!(json, "\"after\":[{}]}}", rows.join(",")).unwrap();
        std::fs::write(args.plots.join(project.replace('/', "_") + ".json"), json)
            .map_err(|e| e.to_string())?;
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

    fn at(x: f32, z: f32) -> Candidate {
        Candidate {
            position: Vec3::new(x, 0.0, z),
            yaw: 0.0,
        }
    }

    /// A strip 0..100 m with a spawn at its west end: the next goes where
    /// it brings the most of the strip nearer, its far half, not its end;
    /// then the gaps between.
    #[test]
    fn new_places_spread_over_the_inner_area() {
        let cells: Vec<(f32, f32)> = (0..50).map(|i| (i as f32 * 2.0 + 1.0, 0.0)).collect();
        let candidates: Vec<Candidate> = (0..=10).map(|i| at(i as f32 * 10.0, 0.0)).collect();
        let existing = [Vec3::new(0.0, 0.0, 0.0)];
        let chosen = choose(&candidates, &existing, &cells, 1);
        let x = candidates[chosen[0]].position.x;
        assert!((50.0..=80.0).contains(&x), "{x}");
        let three = choose(&candidates, &existing, &cells, 3);
        let mut xs: Vec<f32> = three.iter().map(|&i| candidates[i].position.x).collect();
        xs.sort_by(f32::total_cmp);
        assert!(xs.windows(2).all(|w| w[1] - w[0] >= 20.0), "{xs:?}");
    }

    /// No distance is kept: with only places beside a spawn, they are taken,
    /// every candidate at most once.
    #[test]
    fn a_place_beside_a_spawn_is_taken_when_nothing_else_is_left() {
        let cells = [(0.0, 0.0), (2.0, 0.0)];
        let candidates = [at(1.0, 0.0), at(0.5, 0.0)];
        let existing = [Vec3::new(0.0, 0.0, 0.0)];
        let mut both = choose(&candidates, &existing, &cells, 3);
        both.sort_unstable();
        assert_eq!(both, vec![0, 1]);
    }

    #[test]
    fn replaced_ids_go_to_the_nearest_new_places() {
        let old = [
            (7, Vec3::new(0.0, 0.0, 0.0)),
            (9, Vec3::new(100.0, 0.0, 0.0)),
        ];
        let new = [
            Vec3::new(90.0, 0.0, 0.0),
            Vec3::new(50.0, 0.0, 0.0),
            Vec3::new(5.0, 0.0, 0.0),
        ];
        // The place left over is a new point.
        assert_eq!(assign(&old, &new), vec![Some(9), None, Some(7)]);
    }

    #[test]
    fn a_fence_holds_its_inside() {
        let square = [[0.0, 0.0], [10.0, 0.0], [10.0, 10.0], [0.0, 10.0]];
        assert!(within(&square, 5.0, 5.0));
        assert!(!within(&square, 15.0, 5.0));
        assert!(!within(&square, -1.0, 5.0));
    }
}
