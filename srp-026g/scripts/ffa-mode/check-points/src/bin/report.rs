//! Every spawn of a project checked where it stands, none moved: London
//! (v0.32)'s owner's spawns and Free for All picks on 0.32a4's collision
//! (the owner's rule, 2026-11-21: re-validated and reported, none moved).
//!
//! ```text
//! report --db <resources.db> --patch <resources.db.patch_12> ... \
//!     --project v032a_level_09/day_4 [--project v032a_level_09/day_8 ...]
//! ```
//!
//! One TSV row a spawn on stdout: the spawn check (`Map::check`: standing
//! floor within 0.5 m, body clear, a reachable floor of the walk grid) and
//! the nearest harmful zone within 2 m.
use ffa_check_points::{Map, Verdict};
use mapdata::{Materials, Resources, Vec3};
use std::{path::PathBuf, process::ExitCode};

const HARMFUL_MARGIN: f32 = 2.0;

fn run() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let (mut layers, mut projects) = (Vec::new(), Vec::new());
    while let Some(flag) = args.next() {
        let value = args.next().ok_or_else(|| format!("{flag} needs a value"))?;
        match flag.as_str() {
            "--db" | "--patch" => layers.push(PathBuf::from(value)),
            "--project" => projects.push(value),
            _ => return Err(format!("unknown flag {flag}")),
        }
    }
    if layers.is_empty() || projects.is_empty() {
        return Err("--db and at least one --project are required".into());
    }
    let mut resources = Resources::open_layers(&layers).map_err(|e| e.to_string())?;
    let materials = Materials::load(&mut resources).map_err(|e| e.to_string())?;
    println!("project\tteam\tpoint\tx\ty\tz\tverdict\tharmful");
    for project in &projects {
        let map = Map::load(&mut resources, &materials, project)?;
        for s in &map.data.spawns {
            let (verdict, feet) = match map.check(s.position) {
                Verdict::Kept => ("on its ground".to_owned(), s.position),
                Verdict::Nudged { ground } => (
                    format!("its ground {:+.2} m", ground - s.position.y),
                    Vec3::new(s.position.x, ground, s.position.z),
                ),
                Verdict::Dropped(why) => (format!("fails: {why}"), s.position),
            };
            let harmful = match map.harmful_zone(feet, HARMFUL_MARGIN) {
                Some((name, d)) => format!("{name} {d:.2} m"),
                None => String::new(),
            };
            let p = s.position;
            println!(
                "{project}\t{}\t{}\t{:.3}\t{:.3}\t{:.3}\t{verdict}\t{harmful}",
                s.team, s.point_id, p.x, p.y, p.z
            );
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
