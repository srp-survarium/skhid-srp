//! A map's Free for All spawns at the owner's picks (2026-11-14: Rudnya, the
//! console `where` at each place he thinks a good spawn, 56 of them:
//! `private/spawn-examples/rudnya-2026-11-14.tsv`, `n x y z yaw`).
//!
//! ```text
//! picks --db <resources.db> --patch <resources.db.patch_12> ... \
//!     --points <tsv> [--roofs 49,8,10,42] \
//!     --project level_01/day_8 [--project level_01/evening_8 ...]
//! ```
//!
//! Each pick is put on its ground by the spawn check (`Map::check`: standing
//! floor within 0.5 m, body clear, a reachable floor of the walk grid) and
//! kept 2 m from harmful zones; a pick failing either is dropped. Of the rest,
//! the most that are 12 m apart in plan (to_center's spacing) are kept: among
//! picks closer than that, each group keeps its largest set, then the one
//! with the most picks that hold the spawn rule (`cover`), then the lowest
//! numbers. They face as the owner faced: the rotation `[0, -yaw, 0]` (the
//! `where` yaw turns +z towards +x; the engine's rotation_y the other way:
//! `mapdata::math::heading`).
//!
//! The kept picks, in their numbers' order, take the project's solo spawns'
//! point ids in order (only position and rotation change, as
//! `ffa-center-1`); picks past those get new points with the ids after the
//! project's last; solo spawns left over (the highest ids, the project's last
//! entries) go. `--roofs`: after them, a new point on the roof of the
//! building at each of these picks ([`roof`]), its facing the longest view;
//! kept under the spacing (the owner's), the nearest spawn said. Prints
//! checked.tsv rows (`point` the point id, verdict `moved`, `added`, `roof`
//! or `removed`: make_picks_edits.py makes them an edit list) and, on stderr,
//! each dropped pick and why.
use ffa_check_points::cover::{Area, Placement, Sight};
use ffa_check_points::{Map, Verdict};
use mapdata::{Materials, Resources, Vec3};
use std::{path::PathBuf, process::ExitCode};

const SPACING: f32 = 12.0;
const HAZARD_MARGIN: f32 = 2.0;
/// How far ahead the facing is looked along, for the reason column.
const SIGHT: f32 = 20.0;
/// The largest group of picks closer than SPACING searched whole.
const GROUP: usize = 20;

struct Pick {
    n: u32,
    at: Vec3,
    yaw: f32,
}

/// A pick on its ground, with its place by the rule.
struct Placed {
    n: u32,
    feet: Vec3,
    yaw: f32,
    ground: String,
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

/// The picks to keep (indices into `points`): in each group of picks joined
/// by distances under `spacing`, its largest set `spacing` apart, then the
/// one with the most `good`, then the lowest indices.
fn spaced(points: &[Vec3], good: &[bool], spacing: f32) -> Result<Vec<usize>, String> {
    let near = |a: usize, b: usize| points[a].distance_xz(points[b]) < spacing;
    let mut group_of = vec![usize::MAX; points.len()];
    let mut groups: Vec<Vec<usize>> = Vec::new();
    for start in 0..points.len() {
        if group_of[start] != usize::MAX {
            continue;
        }
        let mut members = vec![start];
        group_of[start] = groups.len();
        let mut next = 0;
        while next < members.len() {
            let a = members[next];
            next += 1;
            let id = groups.len();
            for (b, group) in group_of.iter_mut().enumerate() {
                if *group == usize::MAX && near(a, b) {
                    *group = id;
                    members.push(b);
                }
            }
        }
        members.sort_unstable();
        groups.push(members);
    }
    let mut kept = Vec::new();
    for members in groups {
        if members.len() > GROUP {
            return Err(format!("{} picks closer than {spacing} m in one group", members.len()));
        }
        // (size, good, -sum of indices: the lowest first), mask
        let mut best: Option<((usize, usize, i64), u32)> = None;
        for mask in 1u32..(1 << members.len()) {
            let chosen: Vec<usize> = (0..members.len())
                .filter(|i| mask & (1 << i) != 0)
                .map(|i| members[i])
                .collect();
            if chosen
                .iter()
                .enumerate()
                .any(|(i, &a)| chosen[i + 1..].iter().any(|&b| near(a, b)))
            {
                continue;
            }
            let key = (
                chosen.len(),
                chosen.iter().filter(|&&i| good[i]).count(),
                -(chosen.iter().map(|&i| i as i64).sum::<i64>()),
            );
            if best.is_none_or(|(k, _)| key > k) {
                best = Some((key, mask));
            }
        }
        let (_, mask) = best.ok_or("an empty group")?;
        kept.extend(
            (0..members.len())
                .filter(|i| mask & (1 << i) != 0)
                .map(|i| members[i]),
        );
    }
    kept.sort_unstable();
    Ok(kept)
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
}

/// A spawn on the roof of the building at an owner's pick (the owner,
/// 2026-11-14: "for 49, 8, 10 and 42 we need spawn points on top of those
/// builings (roofs)"; "those are silly spawn points usually unreachable, but
/// they make sense in general").
struct Roof {
    over: u32,
    feet: Vec3,
    yaw: f32,
    /// Its floor over the pick's.
    height: f32,
    /// A floor of the map's walk grid: one players get to from the spawns.
    on_foot: bool,
    down: Down,
    /// The most its floor rises or falls a metre round it, in degrees.
    slope: f32,
    /// The roofs nearer the pick passed over, and why.
    skipped: Vec<String>,
}

/// The roof spawn over (or beside) `pick`: on the roof nearest the pick in
/// plan that has one (a roof: the floors, joined by walking, of a walk grid of
/// the map's walker mesh round it, every floor, not only those players get to,
/// at least ROOF_ABOVE over the pick with the open sky over them), a floor
/// ROOF_FLAT from its edges and sloping under 20 degrees (35 where the roof
/// has none), body clear and 2 m from harmful zones, with a way down by the
/// walk model's links (drops of 5 m at most) to a floor the map's players
/// walk; of those, the one farthest in plan from every spawn of `set`.
fn roof(map: &Map, pick: &Placed, set: &[Vec3]) -> Result<Roof, String> {
    use mapdata::{Body, LinkKind, WalkGrid};
    let at = pick.feet;
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
    order.sort_by(|&a, &b| nearest[a].0.total_cmp(&nearest[b].0));
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
        let mut candidates: Vec<u32> = (0..local.len() as u32)
            .filter(|&n| region[n as usize] && slope(n).is_some_and(|s| s <= steepest))
            .filter(|&n| {
                let p = local.position(n);
                map.blocked(p).is_none() && !map.harmful(p, HAZARD_MARGIN)
            })
            .collect();
        // The farthest from every spawn of `set` with a way down.
        candidates.sort_by(|&a, &b| spacing(local.position(b)).total_cmp(&spacing(local.position(a))));
        let (d, h) = nearest[id];
        match candidates.iter().find_map(|&n| way_down(n).map(|drop| (n, drop))) {
            Some((n, drop)) => {
                found = Some((n, drop, slope(n).unwrap_or(0.0)));
                break;
            }
            None if candidates.is_empty() => {
                skipped.push(format!("the roof at {h:.1} m, {d:.1} m off: no flat, clear floor"))
            }
            None => skipped.push(format!(
                "the roof at {h:.1} m, {d:.1} m off: no way down a player survives (drops of 5 m at most)"
            )),
        }
    }
    let (node, drop, slope) = found.ok_or_else(|| {
        format!("pick {}: no roof spawn within {ROOF_REACH} m ({})", pick.n, skipped.join("; "))
    })?;
    let feet = local.position(node);
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
        over: pick.n,
        feet,
        yaw,
        height: feet.y - at.y,
        on_foot: walked(feet),
        down: drop.map_or(Down::Walks, Down::Drops),
        slope: slope.atan().to_degrees(),
        skipped,
    })
}

fn run() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let (mut layers, mut projects, mut points, mut roofs) = (Vec::new(), Vec::new(), None, Vec::<u32>::new());
    while let Some(flag) = args.next() {
        let value = args.next().ok_or_else(|| format!("{flag} needs a value"))?;
        match flag.as_str() {
            "--db" | "--patch" => layers.push(PathBuf::from(value)),
            "--project" => projects.push(value),
            "--points" => points = Some(PathBuf::from(value)),
            "--roofs" => {
                roofs = value
                    .split(',')
                    .map(|n| n.parse().map_err(|_| format!("bad --roofs {value}")))
                    .collect::<Result<_, _>>()?
            }
            _ => return Err(format!("unknown flag {flag}")),
        }
    }
    let picks = read(&points.ok_or("--points is required")?)?;
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
            .filter(|s| s.team == 2)
            .map(|s| s.point_id)
            .collect();
        ids.sort_unstable();
        let mut placed = Vec::new();
        for pick in &picks {
            let (feet, ground) = match map.check(pick.at) {
                Verdict::Kept => (pick.at, "on its ground".to_owned()),
                Verdict::Nudged { ground } => (
                    Vec3::new(pick.at.x, ground, pick.at.z),
                    format!("ground {:+.3} m", ground - pick.at.y),
                ),
                Verdict::Dropped(why) => {
                    eprintln!("{project}: pick {} dropped: {why}", pick.n);
                    continue;
                }
            };
            if let Some((zone, d)) = map.harmful_zone(feet, HAZARD_MARGIN) {
                eprintln!(
                    "{project}: pick {} dropped: {d:.2} m from harmful zone {zone} (under {HAZARD_MARGIN} m)",
                    pick.n
                );
                continue;
            }
            placed.push(Placed {
                n: pick.n,
                feet,
                yaw: pick.yaw,
                ground,
                place: Placement::of(&sight, &area, feet),
            });
        }
        let feet: Vec<Vec3> = placed.iter().map(|p| p.feet).collect();
        let good: Vec<bool> = placed.iter().map(|p| p.place.holds()).collect();
        let kept = spaced(&feet, &good, SPACING)?;
        for (i, p) in placed.iter().enumerate().filter(|(i, _)| !kept.contains(i)) {
            let (near, d) = kept
                .iter()
                .map(|&k| (placed[k].n, p.feet.distance_xz(placed[k].feet)))
                .min_by(|a, b| a.1.total_cmp(&b.1))
                .unwrap_or((0, f32::INFINITY));
            eprintln!(
                "{project}: pick {} dropped: {d:.1} m from pick {near} (spacing {SPACING} m); it {} the rule ({})",
                p.n,
                if good[i] { "holds" } else { "fails" },
                p.place.cover.name()
            );
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
            println!(
                "{project}\t{id}\t{}\t{}\t{}\t0\t{}\t0\t20\t0\t{verdict}\t\tthe owner's pick {} (where yaw {:.1}, {}); body clear; harmful zones >=2m; spacing >=12m; cover {}; {rule}; {ahead} m clear ahead",
                p.feet.x,
                p.feet.y,
                p.feet.z,
                rotation_y(p.yaw),
                p.n,
                p.yaw,
                p.ground,
                place.cover.name()
            );
        }
        for id in ids.iter().skip(kept.len()) {
            let s = map.data.spawns.iter().find(|s| s.team == 2 && s.point_id == *id).ok_or("lost a spawn")?;
            let p = s.position;
            println!(
                "{project}\t{id}\t{}\t{}\t{}\t{}\t{}\t{}\t20\t0\tremoved\t\tone solo spawn more than the picks kept",
                p.x, p.y, p.z, s.rotation.x, s.rotation.y, s.rotation.z
            );
        }
        // The roof spawns, after the picks: each spaced from every one before it.
        let mut set: Vec<Vec3> = kept.iter().map(|&i| placed[i].feet).collect();
        for n in &roofs {
            let pick = placed
                .iter()
                .find(|p| p.n == *n)
                .ok_or_else(|| format!("--roofs {n}: no such pick on a floor"))?;
            let r = roof(&map, pick, &set)?;
            let (near, plan, solid) = set
                .iter()
                .zip(kept.iter().map(|&i| format!("pick {}", placed[i].n)).chain(roofs.iter().map(|n| format!("the roof spawn over pick {n}"))))
                .map(|(q, name)| (name, q.distance_xz(r.feet), (*q - r.feet).length()))
                .min_by(|a, b| a.1.total_cmp(&b.1))
                .ok_or("no spawn")?;
            let place = Placement::of(&sight, &area, r.feet);
            let down = match r.down {
                Down::Walks => "down on foot".to_owned(),
                Down::Drops(h) => format!("down by a drop of {h:.1} m"),
            };
            println!(
                "{project}\t{next_id}\t{}\t{}\t{}\t0\t{}\t0\t20\t0\troof\t\troof spawn of the building at the owner's pick {} (facing {:.1} as where turns, the longest view); its floor {:.1} m over the pick's, sloping {:.0} deg; {}; {down}; body clear; harmful zones >=2m; nearest spawn {near}: {plan:.1} m in plan, {solid:.1} m in 3D{}; cover {}{}{}",
                r.feet.x,
                r.feet.y,
                r.feet.z,
                rotation_y(r.yaw),
                r.over,
                r.yaw,
                r.height,
                r.slope,
                if r.on_foot { "reached on foot" } else { "not reached on foot" },
                if plan < SPACING { " (under the 12 m spacing: kept, the owner's)" } else { "" },
                place.cover.name(),
                if place.inside { "" } else { "; in the perimeter band" },
                r.skipped.iter().map(|w| format!("; not {w}")).collect::<String>(),
            );
            next_id += 1;
            set.push(r.feet);
        }
        eprintln!(
            "{project}: {} picks, {} on a floor and clear of harmful zones, {} kept 12 m apart, {} of them hold the rule",
            picks.len(),
            placed.len(),
            kept.len(),
            kept.iter().filter(|&&i| good[i]).count()
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

    #[test]
    fn close_picks_keep_the_largest_set_then_those_holding_the_rule() {
        let p = |x: f32| Vec3::new(x, 0.0, 0.0);
        // 0 and 1 are 5 m apart, 1 and 2 too; 0 and 2 10: one of 0+2 or 1
        // (a chain keeps its ends); 3 is alone.
        let points = [p(0.0), p(5.0), p(10.0), p(40.0)];
        assert_eq!(spaced(&points, &[true; 4], 6.0).unwrap(), [0, 2, 3]);
        // A pair: the one holding the rule.
        let pair = [p(0.0), p(5.0)];
        assert_eq!(spaced(&pair, &[false, true], 6.0).unwrap(), [1]);
        assert_eq!(spaced(&pair, &[true, true], 6.0).unwrap(), [0]);
    }
}
