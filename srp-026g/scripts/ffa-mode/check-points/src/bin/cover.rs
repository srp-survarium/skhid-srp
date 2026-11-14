//! The spawn rule (`ffa_check_points::cover`, docs/notes/ffa.md "Cover") on a
//! map's Free for All spawns, on points given, and on a sample of its floors:
//! one TSV row each on stdout.
//!
//! ```text
//! cover --db <resources.db> --patch <resources.db.patch_12> ... \
//!     --project level_01/day_8 [--project <p>@x0,z0,x1,z1 ...] \
//!     [--points level_01/day_8=<tsv of n x y z yaw>] [--floors <metres>] \
//!     [--plots <folder>] [--raw]
//! ```
//!
//! - `spawn` rows: the project's team-2 points where they stand, `yaw` the
//!   console `where`'s heading of their facing (degrees from +z towards +x:
//!   the rotation's y is minus it);
//! - `example` rows: the points of the TSV (a `where` line's feet and yaw),
//!   put on their ground by the spawn check first (`snap`);
//! - `floor` rows: the reachable standing floors, one in each cell of
//!   `--floors` metres in plan (and 3 m in height).
//!
//! Then the rule's measures: the depth in the walkable area and whether it
//! is past the perimeter band; the roof's height over the feet; the nearest
//! wall (distance@heading); the corner (the angle of its two walls' faces,
//! and their headings); the side (the low arc's width@its middle); the
//! cover found (`roof`, `corner`, `side`, `none`) and whether both parts
//! hold. `--raw` adds each ray's distance (`-` for none within the reach),
//! low ring then high ring. `--plots` writes <folder>/<project>.json, the
//! area's depth raster and its band, for a render.
use ffa_check_points::cover::{self, Area, Placement, Sight};
use ffa_check_points::{Map, Verdict};
use mapdata::{Materials, Resources, Vec3};
use std::{collections::BTreeSet, path::PathBuf, process::ExitCode};

struct Args {
    layers: Vec<PathBuf>,
    projects: Vec<(String, Option<[f32; 4]>)>,
    points: Vec<(String, PathBuf)>,
    floors: Option<f32>,
    plots: Option<PathBuf>,
    raw: bool,
}

fn parse() -> Result<Args, String> {
    let mut args = std::env::args().skip(1);
    let mut out = Args {
        layers: Vec::new(),
        projects: Vec::new(),
        points: Vec::new(),
        floors: None,
        plots: None,
        raw: false,
    };
    while let Some(flag) = args.next() {
        if flag == "--raw" {
            out.raw = true;
            continue;
        }
        let value = args.next().ok_or_else(|| format!("{flag} needs a value"))?;
        match flag.as_str() {
            "--db" | "--patch" => out.layers.push(PathBuf::from(value)),
            "--project" => out.projects.push(match value.split_once('@') {
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
            "--points" => {
                let (p, f) = value
                    .split_once('=')
                    .ok_or_else(|| format!("--points {value}: <project>=<tsv>"))?;
                out.points.push((p.to_owned(), PathBuf::from(f)));
            }
            "--floors" => {
                out.floors = Some(value.parse().map_err(|_| format!("bad --floors {value}"))?)
            }
            "--plots" => out.plots = Some(PathBuf::from(value)),
            _ => return Err(format!("unknown flag {flag}")),
        }
    }
    if out.layers.is_empty() || out.projects.is_empty() {
        return Err("--db and at least one --project are required".into());
    }
    Ok(out)
}

/// A point to measure: its set, id, feet, `where` yaw and how it was put on
/// its ground.
struct Point {
    set: &'static str,
    id: String,
    feet: Vec3,
    yaw: Option<f32>,
    snap: String,
}

/// The rows of a points TSV (`n x y z yaw`, a header first).
fn read_points(map: &Map, path: &PathBuf) -> Result<Vec<Point>, String> {
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
                    .ok_or_else(|| format!("{}: bad row {line}", path.display()))
            };
            let at = Vec3::new(num(x)?, num(y)?, num(z)?);
            let (feet, snap) = match map.check(at) {
                Verdict::Kept => (at, "kept".to_owned()),
                Verdict::Nudged { ground } => {
                    (Vec3::new(at.x, ground, at.z), format!("nudged {:+.3}", ground - at.y))
                }
                Verdict::Dropped(why) => (at, format!("dropped: {why}")),
            };
            Ok(Point {
                set: "example",
                id: f[n].to_owned(),
                feet,
                yaw: Some(num(yaw)?),
                snap,
            })
        })
        .collect()
}

fn opt(v: Option<f32>) -> String {
    v.map_or("-".into(), |v| format!("{v:.2}"))
}

fn run() -> Result<(), String> {
    let args = parse()?;
    let mut resources = Resources::open_layers(&args.layers).map_err(|e| e.to_string())?;
    let materials = Materials::load(&mut resources).map_err(|e| e.to_string())?;
    let mut header = "project\tset\tid\tx\ty\tz\tyaw\tsnap\tdepth\tband\tinside\troof\twall\tcorner\tside\tcover\tholds".to_owned();
    if args.raw {
        for ring in ["low", "high"] {
            for k in 0..cover::DIRECTIONS {
                header += &format!("\t{ring}{}", cover::degrees(k));
            }
        }
    }
    println!("{header}");
    for (project, bounds) in &args.projects {
        eprintln!("loading {project}");
        let map = Map::load_in(&mut resources, &materials, project, *bounds)?;
        let sight = Sight::of(&map);
        let area = Area::of(&map, *bounds)?;
        let mut points: Vec<Point> = map
            .data
            .spawns
            .iter()
            .filter(|s| s.team == 2)
            .map(|s| Point {
                set: "spawn",
                id: s.point_id.to_string(),
                feet: s.position,
                yaw: Some((-s.rotation.y.to_degrees()).rem_euclid(360.0)),
                snap: String::new(),
            })
            .collect();
        for (p, file) in &args.points {
            if p == project {
                points.extend(read_points(&map, file)?);
            }
        }
        if let Some(step) = args.floors {
            let mut cells = BTreeSet::new();
            for n in map.standing_floors(*bounds) {
                let p = map.grid.position(n);
                let cell = (
                    (p.x / step).floor() as i32,
                    (p.y / 3.0).floor() as i32,
                    (p.z / step).floor() as i32,
                );
                if cells.insert(cell) {
                    points.push(Point {
                        set: "floor",
                        id: n.to_string(),
                        feet: p,
                        yaw: None,
                        snap: String::new(),
                    });
                }
            }
        }
        let mut holds = [0usize; 2];
        for point in &points {
            let place = Placement::of(&sight, &area, point.feet);
            let s = &place.surroundings;
            let wall = s
                .walls(cover::REACH)
                .first()
                .map_or("-".into(), |w| format!("{:.2}@{:.0}", w.distance, w.heading));
            let corner = s.corner().map_or("-".into(), |(a, b, angle)| {
                format!("{angle:.0}@{:.0}/{:.0}", a.heading, b.heading)
            });
            let side = s
                .side()
                .map_or("-".into(), |(arc, heading)| format!("{arc:.0}@{heading:.0}"));
            let p = point.feet;
            let mut row = format!(
                "{project}\t{}\t{}\t{:.3}\t{:.3}\t{:.3}\t{}\t{}\t{:.1}\t{:.1}\t{}\t{}\t{wall}\t{corner}\t{side}\t{}\t{}",
                point.set,
                point.id,
                p.x,
                p.y,
                p.z,
                point.yaw.map_or("-".into(), |y| format!("{y:.1}")),
                point.snap,
                place.depth,
                area.band(),
                if place.inside { "yes" } else { "no" },
                opt(s.roof),
                place.cover.name(),
                if place.holds() { "yes" } else { "no" },
            );
            if args.raw {
                for t in s.low.iter().chain(&s.high) {
                    row += "\t";
                    row += &opt(*t);
                }
            }
            println!("{row}");
            if point.set == "spawn" {
                holds[usize::from(place.holds())] += 1;
            }
        }
        eprintln!(
            "{project}: depth max {:.1} m, band {:.1} m; {} solo spawns, {} hold the rule",
            area.max,
            area.band(),
            holds[0] + holds[1],
            holds[1]
        );
        if let Some(dir) = &args.plots {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
            let depth: Vec<String> = area
                .depth
                .iter()
                .map(|d| d.map_or("-1".into(), |d| format!("{d:.1}")))
                .collect();
            let json = format!(
                "{{\"project\":\"{project}\",\"cell\":{},\"x0\":{:.3},\"z0\":{:.3},\"width\":{},\"height\":{},\"max\":{:.3},\"band\":{:.3},\"depth\":[{}]}}",
                Area::CELL,
                area.x0,
                area.z0,
                area.width,
                area.height,
                area.max,
                area.band(),
                depth.join(",")
            );
            std::fs::write(dir.join(project.replace('/', "_") + ".json"), json)
                .map_err(|e| e.to_string())?;
        }
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
