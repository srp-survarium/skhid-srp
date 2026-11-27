//! A map's Free for All spawns at the owner's picks (2026-11-14: Rudnya, the
//! console `where` at each place he thinks a good spawn, 56 of them:
//! `private/spawn-examples/rudnya-2026-11-14.tsv`, `n x y z yaw`).
//!
//! ```text
//! picks --db <resources.db> --patch <resources.db.patch_12> ... \
//!     [--roof-place 10=26.8,25.926,2.816] \
//!     --points <tsv> [--roofs 49,8,10,42] [--stranded-ok 13] [--harmful-ok 45] [--as-stood 12] \
//!     [--shift 1.5] [--roof-at 24.563,0.912,30.789] [--team 1] [--add [--cap 128]] \
//!     --project level_01/day_8 [--project level_01/evening_8 ...]
//! ```
//!
//! Each pick is put on its ground by the spawn check (`Map::check`: standing
//! floor within 0.5 m, body clear, a reachable floor of the walk grid) and
//! kept 2 m from harmful zones (but the picks `--harmful-ok` names: the
//! owner's, 2026-11-14, "45 can stay too", 1.5 m from Rudnya's swamp anomaly);
//! a pick failing either is dropped, but one `--as-stood` names that fails the
//! first stays at its own `where` position (the owner, 2026-11-14, "keep all
//! my positions": he stood there; the check's collision has no floor under its
//! centre). `--shift <m>`: a dropped pick goes to the nearest place within
//! that many metres in plan that passes both (picks of one map on its ports,
//! where he did not stand). Every other pick is kept, however near
//! another (the owner, 2026-11-14: "we don't need this rules at all", of the
//! 12 m spacing; the respawn's choice keeps players apart). They face as the
//! owner faced: the rotation `[0, -yaw, 0]` (the `where` yaw turns +z towards
//! +x; the engine's rotation_y the other way: `mapdata::math::heading`).
//!
//! The kept picks, in their numbers' order, take the project's solo spawns'
//! (team 2's; `--team <n>` another team's: London (v0.32)'s red team at the
//! owner's picks, 2026-11-23) point ids in order (only position and rotation change, as
//! `ffa-center-1`); picks past those get new points with the ids after the
//! project's last; solo spawns left over (the highest ids, the project's last
//! entries) go. `--add`: every kept pick is a new point after the project's
//! last, and the team's points stay as they are (more picks on a map whose
//! points are already his: London (v0.32)'s Free for All, 2026-11-27).
//! `--cap <n>` with it: where the team's points and the kept picks pass `n`
//! (128, the most exe patch 142 chooses among), the team's points nearest a
//! pick (those the spawn check drops first, then in 3D the nearest; ties by id) give their ids to picks
//! (`moved`) until they fit, the kept picks taking them in order and the
//! others new ids (Station, 2026-11-27: his 60 "too", over its 96).
//! `--roofs`: after them, a new point on the roof of the
//! building at each of these picks ([`roof`]), its facing the longest view,
//! the nearest spawn said; `--stranded-ok`: those of these picks whose roof
//! may have no way down within 5 m drops. `--roof-at <x>,<y>,<z>` (repeatable):
//! after those, a roof spawn on the building at a `where` place that is no
//! pick (the owner, 2026-11-15, standing in Vostok's checkpoint: "we want the
//! spawn on top of the building those coords have"). `--roof-place
//! <pick>=<x>,<y>,<z>` (repeatable): the roof spawn of the building at that
//! pick is at this place, on its ground by the spawn check, with the roof
//! checks but the flat floor's (body clear, 2 m from harmful zones, a way
//! down): picks of one map on its ports, the map's own roof spawn kept where
//! the port's roof has it (the owner, 2026-11-27, of Rudnya (v0.69): "can
//! rudnya 69 use the same ffa spots as 26?"). Prints
//! checked.tsv rows (`point` the point id, verdict `moved`, `added`, `roof`
//! or `removed`: make_picks_edits.py makes them an edit list) and, on stderr,
//! each dropped pick and why.
use ffa_check_points::cover::{Area, Placement, Sight};
use ffa_check_points::{Map, Verdict};
use mapdata::{Materials, Resources, Vec3};
use std::{path::PathBuf, process::ExitCode};

const HAZARD_MARGIN: f32 = 2.0;
/// `--shift`'s rings and directions round a pick.
const SHIFT_STEP: f32 = 0.25;
const SHIFT_DIRECTIONS: u32 = 16;
/// How far ahead the facing is looked along, for the reason column.
const SIGHT: f32 = 20.0;

struct Pick {
    n: u32,
    at: Vec3,
    yaw: f32,
}

/// Where a pick stands, by the spawn check.
enum Ground {
    On,
    /// Moved this far up (or down) to the check's floor.
    Nudged(f32),
    /// Dropped by the checks there for this; kept this far off (`--shift`).
    Shifted { by: f32, why: String },
    /// Dropped by the check for this; kept where he stood (`--as-stood`).
    AsStood(String),
}

impl Ground {
    fn said(&self) -> String {
        match self {
            Ground::On => "on its ground".to_owned(),
            Ground::Nudged(d) => format!("ground {d:+.3} m"),
            Ground::Shifted { by, why } => format!("shifted {by:.2} m, --shift; at his place: {why}"),
            Ground::AsStood(why) => format!("as he stood, --as-stood; the check: {why}"),
        }
    }

    fn body(&self) -> &'static str {
        match self {
            Ground::AsStood(_) => "body as he stood",
            Ground::On | Ground::Nudged(_) | Ground::Shifted { .. } => "body clear",
        }
    }
}

/// Why a team's point gives its id to a pick (`--cap`).
enum Gave {
    /// The spawn check drops it, for this.
    Fails(String),
    /// It stands this far from the nearest pick.
    Near(f32),
}

impl Gave {
    /// Who gives first: a dropped point, then the nearest a pick.
    fn order(&self, other: &Self) -> std::cmp::Ordering {
        match (self, other) {
            (Gave::Fails(_), Gave::Fails(_)) => std::cmp::Ordering::Equal,
            (Gave::Fails(_), Gave::Near(_)) => std::cmp::Ordering::Less,
            (Gave::Near(_), Gave::Fails(_)) => std::cmp::Ordering::Greater,
            (Gave::Near(a), Gave::Near(b)) => a.total_cmp(b),
        }
    }
}

/// A pick on its ground, with its place by the rule.
struct Placed {
    n: u32,
    feet: Vec3,
    yaw: f32,
    ground: Ground,
    /// Within HAZARD_MARGIN of a harmful zone, kept by `--harmful-ok`.
    harm: Option<String>,
    place: Placement,
}

fn read(path: &PathBuf) -> Result<Vec<Pick>, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut lines = text.lines().filter(|l| !l.trim().is_empty());
    let header: Vec<&str> = lines.next().ok_or("empty points file")?.split('\t').collect();
    let column = |name: &str| {
        header
            .iter()
            .position(|h| *h == name)
            .ok_or_else(|| format!("{}: no column {name}", path.display()))
    };
    let (n, x, y, z, yaw) = (column("n")?, column("x")?, column("y")?, column("z")?, column("yaw")?);
    lines
        .map(|line| {
            let f: Vec<&str> = line.split('\t').collect();
            let num = |i: usize| -> Result<f32, String> {
                f.get(i)
                    .and_then(|v| v.parse().ok())
                    .filter(|v: &f32| v.is_finite())
                    .ok_or_else(|| format!("{}: bad row {line}", path.display()))
            };
            Ok(Pick {
                n: f
                    .get(n)
                    .and_then(|v| v.parse().ok())
                    .ok_or_else(|| format!("{}: bad row {line}", path.display()))?,
                at: Vec3::new(num(x)?, num(y)?, num(z)?),
                yaw: num(yaw)?,
            })
        })
        .collect()
}

/// The rotation's y of a `where` yaw (degrees), in (-pi, pi].
fn rotation_y(yaw: f32) -> f32 {
    let a = (-yaw).to_radians().rem_euclid(std::f32::consts::TAU);
    if a > std::f32::consts::PI {
        a - std::f32::consts::TAU
    } else {
        a
    }
}

/// How high over a pick's floor a roof's floor is at least.
const ROOF_ABOVE: f32 = 2.2;
/// How far in plan from a pick its building's roof is looked for.
const ROOF_REACH: f32 = 12.0;
/// A roof spawn's floor goes on this far round it (no edge), rising or
/// falling at most this much a metre: 20 degrees where the roof has such a
/// floor (Rudnya's flat roofs slope about 10), else 35 (its pitched roofs, about
/// 30; the walk model stands on 60).
const ROOF_FLAT: f32 = 1.0;
const ROOF_SLOPES: [f32; 2] = [0.364, 0.700];
/// How far a roof spawn's facing looks.
const ROOF_SIGHT: f32 = 40.0;

/// How a player gets off a roof spawn to the floors the map's players walk.
enum Down {
    /// On foot (stairs, a ramp, steps under 0.6 m).
    Walks,
    /// With drops, the highest this one (the walk model takes 5 m at most).
    Drops(f32),
    /// None within 5 m drops: on a roof `--stranded-ok` names (the owner,
    /// 2026-11-14, of Vostok's at 13: "set one spawn on top of the building
    /// where 13 is").
    Stranded,
}

/// A spawn on the roof of the building at an owner's pick (the owner,
/// 2026-11-14: "for 49, 8, 10 and 42 we need spawn points on top of those
/// builings (roofs)"; "those are silly spawn points usually unreachable, but
/// they make sense in general").
struct Roof {
    /// Whose building: `pick 13`, or a `--roof-at` place.
    of: String,
    feet: Vec3,
    yaw: f32,
    /// Its floor over the pick's.
    height: f32,
    /// A floor of the map's walk grid: one players get to from the spawns.
    on_foot: bool,
    down: Down,
    /// The most its floor rises or falls a metre round it, in degrees; None
    /// with an edge or a step within the metre (a `--roof-place`).
    slope: Option<f32>,
    /// The roofs nearer the pick passed over, and why.
    skipped: Vec<String>,
}

/// The roof spawn over (or beside) `at`, a pick's feet or a `--roof-at`
/// place, named `of`: on the roof nearest it in
/// plan that has one (a roof: the floors, joined by walking, of a walk grid of
/// the map's walker mesh round it, every floor, not only those players get to,
/// at least ROOF_ABOVE over the pick with the open sky over them), a floor
/// ROOF_FLAT from its edges and sloping under 20 degrees (35 where the roof
/// has none), body clear and 2 m from harmful zones, with a way down by the
/// walk model's links (drops of 5 m at most) to a floor the map's players
/// walk; of those, the one farthest in plan from every spawn of `set` (a
/// preference for spread, no minimum). With `stranded_ok`, the nearest roof
/// with such a floor, a way down or not. With `place` (`--roof-place`), that
/// place on its ground, on the roof it is on, its floor's slope not asked.
fn roof(map: &Map, at: Vec3, of: &str, set: &[Vec3], stranded_ok: bool, place: Option<Vec3>) -> Result<Roof, String> {
    use mapdata::{Body, LinkKind, WalkGrid};
    let r = ROOF_REACH + 13.0;
    let local = WalkGrid::build(
        &map.data.walker_triangles(),
        [at.x - r, at.z - r, at.x + r, at.z + r],
        mapdata::CELL,
        Body::default(),
    );
    let roofs: Vec<bool> = (0..local.len() as u32)
        .map(|n| {
            let p = local.position(n);
            local.can_stand(n)
                && local.headroom(n) > ROOF_SIGHT
                && p.y >= at.y + ROOF_ABOVE
                && p.distance_xz(at) <= ROOF_REACH
        })
        .collect();
    // The roofs: open-sky floors joined by walking, the nearest the pick first.
    let mut label = vec![usize::MAX; local.len()];
    let mut nearest: Vec<(f32, f32)> = Vec::new(); // (distance in plan, height) of each roof
    for start in (0..local.len() as u32).filter(|&n| roofs[n as usize]) {
        if label[start as usize] != usize::MAX {
            continue;
        }
        let id = nearest.len();
        nearest.push((f32::INFINITY, 0.0));
        let mut stack = vec![start];
        while let Some(n) = stack.pop() {
            if label[n as usize] != usize::MAX {
                continue;
            }
            label[n as usize] = id;
            let q = local.position(n);
            if q.distance_xz(at) < nearest[id].0 {
                nearest[id] = (q.distance_xz(at), q.y);
            }
            stack.extend(
                local
                    .links(n)
                    .filter(|&(m, kind)| kind == LinkKind::Walk && roofs[m as usize] && label[m as usize] == usize::MAX)
                    .map(|(m, _)| m),
            );
        }
    }
    let mut order: Vec<usize> = (0..nearest.len()).collect();
    // As near, the lower first: over one place, the building's roof before
    // a surface far above it (Vostok v0.11's border box ends 140 m up).
    order.sort_by(|&a, &b| nearest[a].0.total_cmp(&nearest[b].0).then(nearest[a].1.total_cmp(&nearest[b].1)));
    // `--roof-place`: the place on its ground, and the roof it is on alone.
    let pinned = match place {
        Some(p) => {
            let feet = match map.check(p) {
                Verdict::Kept => p,
                Verdict::Nudged { ground } => Vec3::new(p.x, ground, p.z),
                Verdict::Dropped(why) => return Err(format!("{of}: --roof-place {} {} {}: {why}", p.x, p.y, p.z)),
            };
            let node = local
                .locate(feet, 0.3)
                .filter(|&n| (local.position(n).y - feet.y).abs() < 0.3 && label[n as usize] != usize::MAX)
                .ok_or_else(|| format!("{of}: --roof-place {} {} {}: on no roof", p.x, p.y, p.z))?;
            order = vec![label[node as usize]];
            Some((node, feet))
        }
        None => None,
    };
    let spacing = |p: Vec3| set.iter().map(|q| q.distance_xz(p)).fold(f32::INFINITY, f32::min);
    // The map's own floor there: players get to it.
    let walked = |p: Vec3| {
        map.grid
            .locate(p, 0.3)
            .is_some_and(|m| (map.grid.position(m).y - p.y).abs() < 0.3)
    };
    let mut skipped = Vec::new();
    let mut found = None;
    for id in order {
        let region: Vec<bool> = label.iter().map(|&l| l == id).collect();
        // Its slope: the most it rises or falls a metre, None at an edge.
        let slope = |n: u32| -> Option<f32> {
            let p = local.position(n);
            (0..16).try_fold(0f32, |most, k| {
                let a = k as f32 * std::f32::consts::TAU / 16.0;
                let m = local.locate(p + Vec3::new(a.sin(), 0.0, a.cos()) * ROOF_FLAT, 0.2)?;
                region[m as usize].then(|| most.max((local.position(m).y - p.y).abs() / ROOF_FLAT))
            })
        };
        let steepest = ROOF_SLOPES
            .into_iter()
            .find(|&most| (0..local.len() as u32).any(|n| region[n as usize] && slope(n).is_some_and(|s| s <= most)))
            .unwrap_or(0.0);
        // The way down from `from`: by the local grid's links to the first
        // floor players walk off the roof; the highest drop on it (None: on
        // foot).
        let way_down = |from: u32| -> Option<Option<f32>> {
            let mut parent: Vec<Option<(u32, LinkKind)>> = vec![None; local.len()];
            let mut seen = vec![false; local.len()];
            let mut queue = std::collections::VecDeque::from([from]);
            seen[from as usize] = true;
            let end = loop {
                let n = queue.pop_front()?;
                if label[n as usize] == usize::MAX && walked(local.position(n)) {
                    break n;
                }
                for (m, kind) in local.links(n) {
                    if !std::mem::replace(&mut seen[m as usize], true) {
                        parent[m as usize] = Some((n, kind));
                        queue.push_back(m);
                    }
                }
            };
            let (mut n, mut drop) = (end, None::<f32>);
            while let Some((before, kind)) = parent[n as usize] {
                if kind == LinkKind::Drop {
                    let h = local.position(before).y - local.position(n).y;
                    drop = Some(drop.map_or(h, |d| d.max(h)));
                }
                n = before;
            }
            Some(drop)
        };
        let mut candidates: Vec<u32> = match pinned {
            Some((node, feet)) => vec![node].into_iter().filter(|_| !map.harmful(feet, HAZARD_MARGIN)).collect(),
            None => (0..local.len() as u32)
                .filter(|&n| region[n as usize] && slope(n).is_some_and(|s| s <= steepest))
                .filter(|&n| {
                    let p = local.position(n);
                    map.blocked(p).is_none() && !map.harmful(p, HAZARD_MARGIN)
                })
                .collect(),
        };
        // The farthest from every spawn of `set` with a way down.
        candidates.sort_by(|&a, &b| spacing(local.position(b)).total_cmp(&spacing(local.position(a))));
        let (d, h) = nearest[id];
        match candidates.iter().find_map(|&n| way_down(n).map(|drop| (n, drop))) {
            Some((n, drop)) => {
                found = Some((n, drop.map_or(Down::Walks, Down::Drops), slope(n)));
                break;
            }
            None if candidates.is_empty() => skipped.push(match pinned {
                Some(_) => format!("the place on the roof at {h:.1} m: under {HAZARD_MARGIN} m from a harmful zone"),
                None => format!("the roof at {h:.1} m, {d:.1} m off: no flat, clear floor"),
            }),
            None if stranded_ok => {
                found = Some((candidates[0], Down::Stranded, slope(candidates[0])));
                break;
            }
            None => skipped.push(format!(
                "the roof at {h:.1} m, {d:.1} m off: no way down a player survives (drops of 5 m at most)"
            )),
        }
    }
    let (node, down, slope) = found.ok_or_else(|| {
        format!("{of}: no roof spawn within {ROOF_REACH} m ({})", skipped.join("; "))
    })?;
    let feet = pinned.map_or_else(|| local.position(node), |(_, feet)| feet);
    // Facing: the longest view at eye height, then towards the play area.
    let eye = feet + Vec3::new(0.0, 1.6, 0.0);
    let toward = Vec3::new(-feet.x, 0.0, -feet.z);
    let yaw = (0..32)
        .map(|k| {
            let w = k as f32 * 360.0 / 32.0;
            let d = Vec3::new(w.to_radians().sin(), 0.0, w.to_radians().cos());
            let sight = map.rays.ray_cast(eye, d, ROOF_SIGHT).unwrap_or(ROOF_SIGHT);
            (w, sight, d.x * toward.x + d.z * toward.z)
        })
        .max_by(|a, b| a.1.total_cmp(&b.1).then(a.2.total_cmp(&b.2)))
        .map_or(0.0, |(w, _, _)| w);
    Ok(Roof {
        of: of.to_owned(),
        feet,
        yaw,
        height: feet.y - at.y,
        on_foot: walked(feet),
        down,
        slope: slope.map(|s| s.atan().to_degrees()),
        skipped,
    })
}

fn run() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let (mut layers, mut projects, mut points) = (Vec::new(), Vec::new(), None);
    let (mut roofs, mut harmful_ok, mut as_stood) = (Vec::<u32>::new(), Vec::<u32>::new(), Vec::<u32>::new());
    let mut stranded_ok = Vec::<u32>::new();
    let mut roof_at = Vec::<Vec3>::new();
    let mut roof_place = Vec::<(u32, Vec3)>::new();
    let mut shift = 0f32;
    let mut team = 2i32;
    let mut add = false;
    let mut cap = None::<usize>;
    let numbers = |value: &str, flag: &str| -> Result<Vec<u32>, String> {
        value
            .split(',')
            .map(|n| n.parse().map_err(|_| format!("bad {flag} {value}")))
            .collect()
    };
    while let Some(flag) = args.next() {
        if flag == "--add" {
            add = true;
            continue;
        }
        let value = args.next().ok_or_else(|| format!("{flag} needs a value"))?;
        match flag.as_str() {
            "--db" | "--patch" => layers.push(PathBuf::from(value)),
            "--project" => projects.push(value),
            "--points" => points = Some(PathBuf::from(value)),
            "--roofs" => roofs = numbers(&value, &flag)?,
            "--harmful-ok" => harmful_ok = numbers(&value, &flag)?,
            "--as-stood" => as_stood = numbers(&value, &flag)?,
            "--stranded-ok" => stranded_ok = numbers(&value, &flag)?,
            "--roof-at" => match value.split(',').map(str::parse).collect::<Result<Vec<f32>, _>>().as_deref() {
                Ok(&[x, y, z]) if x.is_finite() && y.is_finite() && z.is_finite() => roof_at.push(Vec3::new(x, y, z)),
                _ => return Err(format!("bad --roof-at {value}")),
            },
            "--roof-place" => match value.split_once('=').and_then(|(n, at)| {
                let at: Vec<f32> = at.split(',').map(str::parse).collect::<Result<_, _>>().ok()?;
                Some((n.parse().ok()?, at))
            }) {
                Some((n, at)) if at.len() == 3 && at.iter().all(|v| v.is_finite()) => {
                    roof_place.push((n, Vec3::new(at[0], at[1], at[2])))
                }
                _ => return Err(format!("bad --roof-place {value}")),
            },
            "--team" => team = value.parse().map_err(|_| format!("bad --team {value}"))?,
            "--cap" => cap = Some(value.parse().map_err(|_| format!("bad --cap {value}"))?),
            "--shift" => {
                shift = value
                    .parse()
                    .ok()
                    .filter(|m: &f32| m.is_finite() && *m >= 0.0)
                    .ok_or_else(|| format!("bad --shift {value}"))?
            }
            _ => return Err(format!("unknown flag {flag}")),
        }
    }
    let picks = read(&points.ok_or("--points is required")?)?;
    if cap.is_some() && !add {
        return Err("--cap is --add's".into());
    }
    if let Some((n, _)) = roof_place.iter().find(|(n, _)| !roofs.contains(n)) {
        return Err(format!("--roof-place {n}: not one of --roofs"));
    }
    if layers.is_empty() || projects.is_empty() {
        return Err("--db and at least one --project are required".into());
    }
    let mut resources = Resources::open_layers(&layers).map_err(|e| e.to_string())?;
    let materials = Materials::load(&mut resources).map_err(|e| e.to_string())?;
    println!(
        "project\tpoint\tx\ty\tz\trx\try\trz\tenemy_check_radius\tinvulnerability_time\tverdict\tground\treason"
    );
    for project in &projects {
        eprintln!("loading {project}");
        let map = Map::load(&mut resources, &materials, project)?;
        let sight = Sight::of(&map);
        let area = Area::of(&map, None)?;
        let mut ids: Vec<i32> = map
            .data
            .spawns
            .iter()
            .filter(|s| s.team == team)
            .map(|s| s.point_id)
            .collect();
        ids.sort_unstable();
        if add {
            ids.clear();
        }
        // `--shift`: the nearest place within `shift` m in plan of a pick
        // the checks drop where it passes them all (rings 0.25 m apart).
        let shifted = |at: Vec3| -> Option<(Vec3, f32)> {
            let rings = (shift / SHIFT_STEP).floor() as u32;
            (1..=rings).find_map(|ring| {
                let r = ring as f32 * SHIFT_STEP;
                (0..SHIFT_DIRECTIONS).find_map(|k| {
                    let a = k as f32 * std::f32::consts::TAU / SHIFT_DIRECTIONS as f32;
                    let q = at + Vec3::new(a.sin() * r, 0.0, a.cos() * r);
                    let feet = match map.check(q) {
                        Verdict::Kept => q,
                        Verdict::Nudged { ground } => Vec3::new(q.x, ground, q.z),
                        Verdict::Dropped(_) => return None,
                    };
                    (!map.harmful(feet, HAZARD_MARGIN)).then_some((feet, r))
                })
            })
        };
        let mut placed = Vec::new();
        for pick in &picks {
            let (mut feet, mut ground) = match map.check(pick.at) {
                Verdict::Kept => (pick.at, Ground::On),
                Verdict::Nudged { ground } => (
                    Vec3::new(pick.at.x, ground, pick.at.z),
                    Ground::Nudged(ground - pick.at.y),
                ),
                Verdict::Dropped(why) if as_stood.contains(&pick.n) => {
                    eprintln!("{project}: pick {} kept as he stood (--as-stood): {why}", pick.n);
                    (pick.at, Ground::AsStood(why))
                }
                Verdict::Dropped(why) => match shifted(pick.at) {
                    Some((feet, by)) => {
                        eprintln!("{project}: pick {} shifted {by:.2} m: {why}", pick.n);
                        (feet, Ground::Shifted { by, why })
                    }
                    None => {
                        eprintln!("{project}: pick {} dropped: {why}", pick.n);
                        continue;
                    }
                },
            };
            let mut near_harm = map.harmful_zone(feet, HAZARD_MARGIN);
            if let Some((zone, d)) = near_harm {
                let why = format!("{d:.2} m from harmful zone {zone} (under {HAZARD_MARGIN} m)");
                if harmful_ok.contains(&pick.n) {
                    eprintln!("{project}: pick {} kept {d:.2} m from harmful zone {zone} (--harmful-ok)", pick.n);
                } else if let Some((there, by)) = shifted(pick.at) {
                    eprintln!("{project}: pick {} shifted {by:.2} m: {why}", pick.n);
                    (feet, ground, near_harm) = (there, Ground::Shifted { by, why }, None);
                } else {
                    eprintln!("{project}: pick {} dropped: {why}", pick.n);
                    continue;
                }
            }
            placed.push(Placed {
                n: pick.n,
                feet,
                yaw: pick.yaw,
                ground,
                harm: near_harm.map(|(zone, d)| format!("{d:.2} m from harmful zone {zone}, the owner's")),
                place: Placement::of(&sight, &area, feet),
            });
        }
        let kept: Vec<usize> = (0..placed.len()).collect();
        // `--cap`: the team's points nearest a pick give their ids to picks until all fit.
        let mut gave = std::collections::BTreeMap::<i32, Gave>::new();
        if let Some(cap) = cap {
            let team_points: Vec<_> = map.data.spawns.iter().filter(|s| s.team == team).collect();
            let over = (team_points.len() + kept.len()).saturating_sub(cap);
            if over > team_points.len() {
                return Err(format!("{project}: {} picks pass --cap {cap} alone", kept.len()));
            }
            // A point the spawn check drops first, then the nearest a pick.
            let mut near: Vec<(i32, Gave)> = team_points
                .iter()
                .map(|s| match map.check(s.position) {
                    Verdict::Dropped(why) => (s.point_id, Gave::Fails(why)),
                    Verdict::Kept | Verdict::Nudged { .. } => {
                        let d = kept
                            .iter()
                            .map(|&i| (placed[i].feet - s.position).length())
                            .fold(f32::INFINITY, f32::min);
                        (s.point_id, Gave::Near(d))
                    }
                })
                .collect();
            near.sort_by(|a, b| a.1.order(&b.1).then(a.0.cmp(&b.0)));
            gave.extend(near.into_iter().take(over));
            ids = gave.keys().copied().collect();
        }
        // The solo spawns' ids, then new ones after the project's last point.
        let last = map.data.spawns.iter().map(|s| s.point_id).max().ok_or("no respawn point")?;
        let new_ids = (last + 1..).take(kept.len().saturating_sub(ids.len()));
        let mut next_id = last + 1 + kept.len().saturating_sub(ids.len()) as i32;
        let places = ids.iter().map(|&id| (id, "moved")).chain(new_ids.map(|id| (id, "added")));
        for (&i, (id, verdict)) in kept.iter().zip(places) {
            let p = &placed[i];
            let eye = p.feet + Vec3::new(0.0, 1.6, 0.0);
            let a = p.yaw.to_radians();
            let ahead = map
                .rays
                .ray_cast(eye, Vec3::new(a.sin(), 0.0, a.cos()), SIGHT)
                .map_or(format!(">{SIGHT:.0}"), |t| format!("{t:.1}"));
            let place = &p.place;
            let rule = match (place.cover.holds(), place.inside) {
                (true, true) => "holds the spawn rule".to_owned(),
                (false, true) => "no cover (fails the spawn rule)".to_owned(),
                (true, false) => format!(
                    "in the perimeter band, {:.1} m deep of {:.1} (fails the spawn rule)",
                    place.depth,
                    area.band()
                ),
                (false, false) => "no cover and in the perimeter band (fails the spawn rule)".to_owned(),
            };
            // A solo spawn's radius and protection; a team's points keep their own (the edit
            // list copies them).
            let radii = if team == 2 { "20\t0" } else { "-\t-" };
            println!(
                "{project}\t{id}\t{}\t{}\t{}\t0\t{}\t0\t{radii}\t{verdict}\t\tthe owner's pick {} (where yaw {:.1}, {}); {}; {}; cover {}; {rule}; {ahead} m clear ahead{}",
                p.feet.x,
                p.feet.y,
                p.feet.z,
                rotation_y(p.yaw),
                p.n,
                p.yaw,
                p.ground.said(),
                p.ground.body(),
                p.harm.as_deref().unwrap_or("harmful zones >=2m"),
                place.cover.name(),
                gave.get(&id).map_or(String::new(), |gave| match gave {
                    Gave::Fails(why) => format!("; in the place of the team's point {id}, which the spawn check drops: {why} (--cap)"),
                    Gave::Near(d) => format!("; in the place of the team's point {id}, {d:.1} m from the nearest pick (--cap)"),
                })
            );
        }
        for id in ids.iter().skip(kept.len()) {
            let s = map.data.spawns.iter().find(|s| s.team == team && s.point_id == *id).ok_or("lost a spawn")?;
            let p = s.position;
            println!(
                "{project}\t{id}\t{}\t{}\t{}\t{}\t{}\t{}\t20\t0\tremoved\t\tone spawn of the team more than the picks kept",
                p.x, p.y, p.z, s.rotation.x, s.rotation.y, s.rotation.z
            );
        }
        // The roof spawns, after the picks: each spaced from every one before it.
        let mut set: Vec<Vec3> = kept.iter().map(|&i| placed[i].feet).collect();
        let mut buildings = Vec::new();
        for n in &roofs {
            let pick = placed
                .iter()
                .find(|p| p.n == *n)
                .ok_or_else(|| format!("--roofs {n}: no such pick on a floor"))?;
            let place = roof_place.iter().find(|(m, _)| m == n).map(|&(_, p)| p);
            buildings.push((format!("pick {n}"), pick.feet, stranded_ok.contains(n), place));
        }
        for at in &roof_at {
            buildings.push((format!("place {} {} {}", at.x, at.y, at.z), *at, false, None));
        }
        for (of, at, stranded, pinned) in &buildings {
            let r = roof(&map, *at, of, &set, *stranded, *pinned)?;
            let (near, plan, solid) = set
                .iter()
                .zip(kept.iter().map(|&i| format!("pick {}", placed[i].n)).chain(buildings.iter().map(|(of, ..)| format!("the roof spawn over {of}"))))
                .map(|(q, name)| (name, q.distance_xz(r.feet), (*q - r.feet).length()))
                .min_by(|a, b| a.1.total_cmp(&b.1))
                .ok_or("no spawn")?;
            let place = Placement::of(&sight, &area, r.feet);
            let down = match r.down {
                Down::Walks => "down on foot".to_owned(),
                Down::Drops(h) => format!("down by a drop of {h:.1} m"),
                Down::Stranded => "no way down within 5 m drops (--stranded-ok, the owner's)".to_owned(),
            };
            println!(
                "{project}\t{next_id}\t{}\t{}\t{}\t0\t{}\t0\t20\t0\troof\t\troof spawn of the building at the owner's {} (facing {:.1} as where turns, the longest view); its floor {:.1} m over the pick's, {}; {}; {down}; body clear; harmful zones >=2m; nearest spawn {near}: {plan:.1} m in plan, {solid:.1} m in 3D; cover {}{}{}",
                r.feet.x,
                r.feet.y,
                r.feet.z,
                rotation_y(r.yaw),
                r.of,
                r.yaw,
                r.height,
                match (r.slope, pinned) {
                    (Some(s), None) => format!("sloping {s:.0} deg"),
                    (Some(s), Some(p)) => format!(
                        "at {} {} {} (--roof-place), its floor within 1 m up to {:.2} m higher or lower",
                        p.x,
                        p.y,
                        p.z,
                        s.to_radians().tan() * ROOF_FLAT
                    ),
                    (None, Some(p)) => format!("at {} {} {} (--roof-place), an edge or a step within 1 m", p.x, p.y, p.z),
                    (None, None) => "an edge or a step within 1 m".to_owned(),
                },
                if r.on_foot { "reached on foot" } else { "not reached on foot" },
                place.cover.name(),
                if place.inside { "" } else { "; in the perimeter band" },
                r.skipped.iter().map(|w| format!("; not {w}")).collect::<String>(),
            );
            next_id += 1;
            set.push(r.feet);
        }
        eprintln!(
            "{project}: {} picks, {} kept (on a floor, clear of harmful zones or let be), {} of them hold the rule",
            picks.len(),
            placed.len(),
            placed.iter().filter(|p| p.place.holds()).count()
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
    fn a_where_yaw_turns_into_the_engine_s_rotation() {
        // where's 90 faces +x; the engine's rotation_y turns +z to
        // (-sin a, 0, cos a): +x at a = -pi/2.
        for yaw in [0.0f32, 45.0, 90.0, 123.0, 180.0, 270.0, 359.3] {
            let a = rotation_y(yaw);
            assert!(a > -std::f32::consts::PI - 1e-6 && a <= std::f32::consts::PI + 1e-6);
            let engine = mapdata::math::forward(Vec3::new(0.0, a, 0.0));
            let w = yaw.to_radians();
            let console = Vec3::new(w.sin(), 0.0, w.cos());
            assert!((engine - console).length() < 1e-5, "{yaw}");
            assert!((mapdata::math::heading(console) - a).abs() < 1e-5 || yaw == 180.0, "{yaw}");
        }
    }
}
