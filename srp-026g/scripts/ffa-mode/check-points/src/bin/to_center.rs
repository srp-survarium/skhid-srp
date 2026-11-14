//! Move half of a map's edge FFA spawns to its centre (the owner, 2026-11-13:
//! "I would remove half spawns on perimeter ... and put them in the center").
//!
//! The walkable area is the map's reachable standing floors (the walk grid
//! from its spawns), rasterised at CELL metres, closed by one cell and with its
//! holes filled: its outline is the outer edge of where a player can walk. A
//! cell's depth is its distance from that outline; DEPTH_MAX the deepest.
//! - perimeter: a solo spawn at depth <= BAND * DEPTH_MAX;
//! - centre area: depth >= CORE * DEPTH_MAX, the cells joined to the deepest
//!   (a wide wing's own deep cells are not the centre). Where it cannot take
//!   all the moved spawns, it widens by WIDEN of DEPTH_MAX at a time, down to
//!   the perimeter band's edge.
//!
//! The perimeter spawns, in order of their angle around the area's centroid,
//! lose every other one (the phase whose kept spawns leave the smaller largest
//! gap). As many new spawns go into the centre area, from the same candidates
//! and checks as the London central points (central.rs): reachable standing
//! floor, body clear, harmful zones >= 2 m, a walkable 6 m facing, the
//! farthest from every kept solo spawn and each other first (center.tsv was made
//! with a 12 m minimum; since 2026-11-14 there is none, the owner: "we don't
//! need this rules at all"). A map whose centre has too few is refused.
//!
//! `--below <y>` (run on the projects with the move applied) moves nothing of
//! the perimeter: the solo spawns standing above that height are the ones that
//! go, to centre places not above it. London's rotunda, 14.5: its second
//! storey's floor is at 14.06, its third's at 17.92 (the owner, 2026-11-13:
//! "london shouldn't have spawns on the third floor").
//!
//! The moved spawns keep their ids: the removed ones, by point id, take the
//! new places in the order chosen. Prints them as checked.tsv rows (`point` is
//! the moved spawn's point id: center.tsv, made into an edit list by
//! make_center_edits.py) and writes <plots>/<project>.json (the area, the
//! spawns and what becomes of them, for a review render).
use ffa_check_points::cover::Area;
use ffa_check_points::{Map, Verdict};
use mapdata::{Materials, Resources, Vec3};
use std::{collections::BTreeSet, fmt::Write as _, path::PathBuf, process::ExitCode};

const HAZARD_MARGIN: f32 = 2.0;
const FACING_REACH: f32 = 6.0;
const WIDEN: f32 = 0.05;
/// The perimeter band and the centre area, of the deepest point's depth (the
/// move's own band, as center.tsv was made; the spawn rule's is `cover::BAND`).
const BAND: f32 = 0.3;
const CORE: f32 = 0.5;

#[derive(Clone, Copy)]
struct Candidate {
    position: Vec3,
    yaw: f32,
}

/// to_center's own reading of the walkable area ([`Area`], the lib's).
trait Centre {
    fn centre(&self, core: f32) -> Vec<bool>;
    fn in_cells(&self, cells: &[bool], p: Vec3) -> bool;
    fn angle(&self, p: Vec3) -> f32;
}

impl Centre for Area {
    /// The cells of the centre area: those at least `core` deep connected to
    /// the deepest (a wing's own deep cells are no centre).
    fn centre(&self, core: f32) -> Vec<bool> {
        let (w, h) = (self.width, self.height);
        let deep = |k: usize| self.depth[k].is_some_and(|d| d >= core);
        let mut centre = vec![false; w * h];
        let deepest = (0..w * h)
            .max_by(|&a, &b| self.depth[a].unwrap_or(-1.0).total_cmp(&self.depth[b].unwrap_or(-1.0)));
        let mut stack: Vec<usize> = deepest.into_iter().filter(|&k| deep(k)).collect();
        while let Some(k) = stack.pop() {
            if centre[k] {
                continue;
            }
            centre[k] = true;
            let (i, j) = ((k % w) as i32, (k / w) as i32);
            for (di, dj) in [(-1, 0), (1, 0), (0, -1), (0, 1), (-1, -1), (1, 1), (-1, 1), (1, -1)] {
                let (a, b) = (i + di, j + dj);
                if a >= 0 && b >= 0 && (a as usize) < w && (b as usize) < h {
                    let n = b as usize * w + a as usize;
                    if !centre[n] && deep(n) {
                        stack.push(n);
                    }
                }
            }
        }
        centre
    }

    fn in_cells(&self, cells: &[bool], p: Vec3) -> bool {
        let (i, j) = ((p.x - self.x0) / Area::CELL, (p.z - self.z0) / Area::CELL);
        i >= 0.0
            && j >= 0.0
            && (i as usize) < self.width
            && (j as usize) < self.height
            && cells[j as usize * self.width + i as usize]
    }

    fn angle(&self, p: Vec3) -> f32 {
        (p.z - self.centroid.1).atan2(p.x - self.centroid.0)
    }
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

fn facing(map: &Map, bounds: Option<[f32; 4]>, node: u32, center: Vec3) -> Option<f32> {
    let p = map.grid.position(node);
    let eye = p + Vec3::new(0.0, 1.6, 0.0);
    let toward = center - p;
    (0..32)
        .filter_map(|i| {
            let yaw = i as f32 * std::f32::consts::TAU / 32.0;
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
            let sight = map.rays.ray_cast(eye, dir, 20.0).unwrap_or(20.0);
            // Prefer room to see ahead, then look into the play area on a tie.
            Some((
                mapdata::math::heading(dir),
                sight,
                dir.x * toward.x + dir.z * toward.z,
            ))
        })
        .max_by(|a, b| a.1.total_cmp(&b.1).then(a.2.total_cmp(&b.2)))
        .map(|(yaw, _, _)| yaw)
}

/// central.rs's choice: the candidate farthest from every kept spawn and
/// every one chosen, until `count`; fewer when the candidates run out.
fn choose(points: &[Candidate], existing: &[Vec3], count: usize) -> Vec<usize> {
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
        let Some(next) = (0..points.len())
            .max_by(|&a, &b| distance[a].total_cmp(&distance[b]).then(b.cmp(&a)))
        else {
            break;
        };
        if distance[next] == f32::NEG_INFINITY {
            break;
        }
        chosen.push(next);
        for (i, p) in points.iter().enumerate() {
            distance[i] = distance[i].min(p.position.distance_xz(points[next].position));
        }
        distance[next] = f32::NEG_INFINITY;
    }
    chosen
}

/// Which of the perimeter spawns (in their order round the area) go: every
/// other one, the phase whose kept ones leave the smaller largest gap.
fn every_other(angles: &[f32]) -> Vec<usize> {
    let gap = |kept: &[f32]| {
        if kept.len() < 2 {
            return std::f32::consts::TAU;
        }
        (0..kept.len())
            .map(|i| (kept[(i + 1) % kept.len()] - kept[i]).rem_euclid(std::f32::consts::TAU))
            .fold(0f32, f32::max)
    };
    let mut order: Vec<usize> = (0..angles.len()).collect();
    order.sort_by(|&a, &b| angles[a].total_cmp(&angles[b]));
    let phase = |p: usize| -> (Vec<usize>, f32) {
        let removed: Vec<usize> = order.iter().copied().skip(p).step_by(2).collect();
        let kept: Vec<f32> = order
            .iter()
            .filter(|i| !removed.contains(i))
            .map(|&i| angles[i])
            .collect();
        (removed, gap(&kept))
    };
    let (a, b) = (phase(1), phase(0));
    // Phase 1 removes floor(n/2) of an odd count; phase 0 one more. Keep that
    // count unless phase 0 leaves a strictly smaller gap at the same count.
    if a.0.len() == b.0.len() && b.1 < a.1 { b.0 } else { a.0 }
}

struct Args {
    layers: Vec<PathBuf>,
    projects: Vec<(String, Option<[f32; 4]>)>,
    plots: PathBuf,
    below: Option<f32>,
}

fn parse() -> Result<Args, String> {
    let mut args = std::env::args().skip(1);
    let (mut layers, mut projects) = (Vec::new(), Vec::new());
    let (mut plots, mut below) = (None, None);
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
            "--plots" => plots = Some(PathBuf::from(value)),
            "--below" => below = Some(value.parse().map_err(|_| format!("bad height {value}"))?),
            _ => return Err(format!("unknown flag {flag}")),
        }
    }
    if layers.is_empty() || projects.is_empty() {
        return Err("--db and at least one --project are required".into());
    }
    Ok(Args {
        layers,
        projects,
        plots: plots.ok_or("--plots is required")?,
        below,
    })
}

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
        let map = Map::load_in(&mut resources, &materials, project, *bounds)?;
        let floors = map.standing_floors(*bounds);
        let positions: Vec<Vec3> = floors.iter().map(|&n| map.grid.position(n)).collect();
        let area = Area::new(&positions)?;
        let band = BAND * area.max;
        let solo: Vec<_> = map.data.spawns.iter().filter(|s| s.team == 2).collect();
        let perimeter: Vec<usize> = (0..solo.len())
            .filter(|&i| area.at(solo[i].position) <= band)
            .collect();
        let angles: Vec<f32> = perimeter
            .iter()
            .map(|&i| area.angle(solo[i].position))
            .collect();
        let removed: Vec<usize> = match args.below {
            Some(top) => (0..solo.len()).filter(|&i| solo[i].position.y > top).collect(),
            None => every_other(&angles).into_iter().map(|k| perimeter[k]).collect(),
        };
        let kept: Vec<Vec3> = (0..solo.len())
            .filter(|i| !removed.contains(i))
            .map(|i| solo[i].position)
            .collect();
        let centre = Vec3::new(area.centroid.0, 0.0, area.centroid.1);
        let mut cells = BTreeSet::new();
        let mut candidates = Vec::new();
        for (&node, &p) in floors.iter().zip(&positions) {
            if area.at(p) <= band
                || args.below.is_some_and(|top| p.y > top)
                || !cells.insert((
                    (p.x / 2.0).floor() as i32,
                    (p.y / 2.0).floor() as i32,
                    (p.z / 2.0).floor() as i32,
                ))
                || harmful(&map, p)
            {
                continue;
            }
            let position = match map.check(p) {
                Verdict::Nudged { ground } => Vec3::new(p.x, ground, p.z),
                Verdict::Kept => p,
                Verdict::Dropped(_) => continue,
            };
            if let Some(yaw) = facing(&map, *bounds, node, centre) {
                candidates.push(Candidate { position, yaw });
            }
        }
        // The centre area, widened in steps of WIDEN of the deepest while it
        // cannot take them all, down to the perimeter band's edge.
        let (mut fraction, mut core, mut cells_in, mut central, mut added);
        fraction = CORE;
        loop {
            core = fraction * area.max;
            cells_in = area.centre(core);
            central = candidates
                .iter()
                .copied()
                .filter(|c| area.in_cells(&cells_in, c.position))
                .collect::<Vec<_>>();
            added = choose(&central, &kept, removed.len())
                .into_iter()
                .map(|i| central[i])
                .collect::<Vec<Candidate>>();
            if added.len() == removed.len() || fraction <= BAND {
                break;
            }
            fraction = (fraction - WIDEN).max(BAND);
        }
        let candidates = central;
        eprintln!(
            "{project}: depth max {:.1} m, centre >= {fraction:.2} of it, {} solo, {} perimeter, {} removed, {} candidates, {} added",
            area.max,
            solo.len(),
            perimeter.len(),
            removed.len(),
            candidates.len(),
            added.len()
        );
        if added.len() < removed.len() {
            return Err(format!(
                "{project}: the centre takes {} of the {} moved spawns",
                added.len(),
                removed.len()
            ));
        }
        let mut moved: Vec<i32> = removed.iter().map(|&i| solo[i].point_id).collect();
        moved.sort_unstable();
        // The stored facing, checked again as the STALKER generator does.
        for (i, c) in added.iter().enumerate() {
            let p = c.position;
            if harmful(&map, p) {
                return Err(format!("{project}: new point {i} is near a harmful zone"));
            }
            if let Verdict::Dropped(why) = map.check(p) {
                return Err(format!("{project}: new point {i} failed final check: {why}"));
            }
            let dir = mapdata::math::forward(Vec3::new(0.0, c.yaw, 0.0));
            let start = map.grid.locate(p, 0.6).ok_or("new point lost its floor")?;
            let end = map
                .grid
                .locate(p + dir * FACING_REACH, 0.6)
                .ok_or("new point's facing has no destination")?;
            if !map.grid.can_stand(end)
                || harmful(&map, map.grid.position(end))
                || !map
                    .grid
                    .walks_straight(start, end, FACING_REACH + 0.5, |n| map.grid.can_stand(n))
            {
                return Err(format!("{project}: new point {i}'s stored facing is blocked"));
            }
            println!(
                "{project}\t{}\t{}\t{}\t{}\t0\t{}\t0\t20\t2\tmoved\t\treachable standing floor; body clear; harmful zones >=2m; walkable facing 6m; farthest-first; centre area (depth >= {:.1} m)",
                moved[i], p.x, p.y, p.z, c.yaw, core
            );
        }
        let mut json = String::new();
        let f = |v: f32| format!("{v:.3}");
        write!(
            json,
            "{{\"project\":\"{project}\",\"cell\":{},\"x0\":{},\"z0\":{},\"width\":{},\"height\":{},\"max\":{},\"band\":{},\"core\":{},\"centroid\":[{},{}],",
            Area::CELL,
            f(area.x0),
            f(area.z0),
            area.width,
            area.height,
            f(area.max),
            f(band),
            f(core),
            f(area.centroid.0),
            f(area.centroid.1)
        )
        .unwrap();
        let depth: Vec<String> = area
            .depth
            .iter()
            .map(|d| d.map_or("-1".into(), |d| format!("{d:.1}")))
            .collect();
        let centre: Vec<&str> = cells_in.iter().map(|&c| if c { "1" } else { "0" }).collect();
        write!(
            json,
            "\"depth\":[{}],\"centre\":[{}],\"points\":[",
            depth.join(","),
            centre.join(",")
        )
        .unwrap();
        let mut rows = Vec::new();
        for (i, s) in solo.iter().enumerate() {
            let state = if removed.contains(&i) { "removed" } else { "kept" };
            rows.push(format!(
                "{{\"id\":{},\"state\":\"{state}\",\"x\":{},\"y\":{},\"z\":{},\"yaw\":{},\"depth\":{},\"centre\":{}}}",
                s.point_id,
                f(s.position.x),
                f(s.position.y),
                f(s.position.z),
                f(s.rotation.y),
                f(area.at(s.position)),
                area.in_cells(&cells_in, s.position)
            ));
        }
        for (i, c) in added.iter().enumerate() {
            rows.push(format!(
                "{{\"id\":{},\"state\":\"new\",\"x\":{},\"y\":{},\"z\":{},\"yaw\":{},\"depth\":{},\"centre\":{}}}",
                moved[i],
                f(c.position.x),
                f(c.position.y),
                f(c.position.z),
                f(c.yaw),
                f(area.at(c.position)),
                area.in_cells(&cells_in, c.position)
            ));
        }
        write!(json, "{}],\"candidates\":{}}}", rows.join(","), candidates.len()).unwrap();
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

    #[test]
    fn every_other_perimeter_spawn_goes_and_the_rest_stay_spread() {
        let angles: Vec<f32> = (0..8).map(|i| i as f32 * std::f32::consts::TAU / 8.0).collect();
        let removed = every_other(&angles);
        assert_eq!(removed.len(), 4);
        assert!(removed.windows(2).all(|w| w[1] - w[0] == 2));
        assert_eq!(every_other(&angles[..7]).len(), 3);
        assert!(every_other(&[]).is_empty());
    }

    #[test]
    fn the_area_is_deepest_in_its_middle() {
        let floors: Vec<Vec3> = (0..50)
            .flat_map(|x| (0..50).map(move |z| Vec3::new(x as f32, 0.0, z as f32)))
            .collect();
        let area = Area::new(&floors).unwrap();
        assert!(area.at(Vec3::new(25.0, 0.0, 25.0)) > 20.0);
        assert!(area.at(Vec3::new(1.0, 0.0, 25.0)) < 4.0);
        assert!(area.at(Vec3::new(-50.0, 0.0, 25.0)) == 0.0);
    }
}
