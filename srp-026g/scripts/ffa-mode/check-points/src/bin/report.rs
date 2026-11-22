//! Every spawn of a project checked where it stands, none moved: London
//! (v0.32)'s owner's spawns and Free for All picks on 0.32a4's collision
//! (the owner's rule, 2026-11-21: re-validated and reported, none moved).
//!
//! ```text
//! report --db <resources.db> --patch <resources.db.patch_12> ... \
//!     --project v032a_level_09/day_4 [--project v032a_level_09/day_8 ...]
//! SRP_GAME_DATA=<server-game-data.db> report --db ... --patch ... --team-maps
//! ```
//!
//! One TSV row a spawn on stdout: the spawn check (`Map::check`: standing
//! floor within 0.5 m, body clear, a reachable floor of the walk grid), the
//! nearest harmful zone within 2 m, how far the spawn's facing is clear, how
//! far it turns from the map's centre ([`off_centre`]) and, for a team's
//! spawn, whether it faces a wall or the map's edge
//! ([`facing`]). `--team-maps`: the projects of every playable map of a team
//! mode of the game data `SRP_GAME_DATA` names (the layers must be its
//! archives). A report only: nothing fails on what it finds; stderr counts
//! the team spawns that face a wall.
use ffa_check_points::{Map, Verdict};
use mapdata::{Materials, Resources, Vec3};
use std::{path::PathBuf, process::ExitCode};
use survarium::v026g::{
    game_data,
    maps::{self, GameMode},
};

const HARMFUL_MARGIN: f32 = 2.0;
/// The eye over the feet: `where` says 1.465 standing; the picks' 1.6.
const EYE: f32 = 1.6;
/// How far ahead the facing is looked along.
const SIGHT: f32 = 20.0;
/// A team spawn faces a wall when its facing is shut this near: the owner's
/// 13 London (v0.32) red picks (2026-11-23, "pit of success (move correct
/// direction)") are clear 5.8 m and more; a corridor's far wall or a door
/// across a room is farther than this, a wall a player has to turn from is
/// nearer.
const FACING_CLEAR: f32 = 3.0;

/// What a team spawn's facing meets within [`FACING_CLEAR`].
enum Facing {
    Clear,
    Wall(f32),
    Edge(f32),
}

/// The spawn's facing as the exe reads it (`respawn_point_core::load`
/// 0x2F4580: `create_rotation` of the whole rotation, the yaw of its k row,
/// 0xEE370): `mapdata::math::forward` on the ground plane. How far it is
/// clear from the eye, up to [`SIGHT`], and what it meets within
/// [`FACING_CLEAR`]: the walker mesh (a wall), or the map's box (its edge).
fn facing(map: &Map, feet: Vec3, rotation: Vec3) -> (Option<f32>, Facing) {
    let f = mapdata::math::forward(rotation);
    let flat = Vec3::new(f.x, 0.0, f.z);
    if flat.length() < 1e-3 {
        return (None, Facing::Clear);
    }
    let dir = flat * (1.0 / flat.length());
    let eye = feet + Vec3::new(0.0, EYE, 0.0);
    let hit = map.rays.ray_cast(eye, dir, SIGHT);
    let [x0, z0, x1, z1] = map.data.play_area(0.0);
    let ahead = eye + dir * FACING_CLEAR;
    let verdict = match hit {
        Some(t) if t < FACING_CLEAR => Facing::Wall(t),
        _ if ahead.x < x0 || ahead.x > x1 || ahead.z < z0 || ahead.z > z1 => {
            let out = [(x0 - eye.x) / dir.x, (x1 - eye.x) / dir.x, (z0 - eye.z) / dir.z, (z1 - eye.z) / dir.z];
            Facing::Edge(out.into_iter().filter(|t| *t >= 0.0).fold(FACING_CLEAR, f32::min))
        }
        _ => Facing::Clear,
    };
    (hit, verdict)
}

/// How far the spawn's facing turns from the way to the map's centre (the
/// middle of its box), in degrees: a measure, no rule (the owner's London
/// (v0.32) red picks turn up to 116 from it to face a door).
fn off_centre(map: &Map, feet: Vec3, rotation: Vec3) -> Option<f32> {
    let f = mapdata::math::forward(rotation);
    let [x0, z0, x1, z1] = map.data.play_area(0.0);
    let (cx, cz) = ((x0 + x1) / 2.0 - feet.x, (z0 + z1) / 2.0 - feet.z);
    let (a, b) = ((f.x * f.x + f.z * f.z).sqrt(), (cx * cx + cz * cz).sqrt());
    (a > 1e-3 && b > 1e-3).then(|| ((f.x * cx + f.z * cz) / (a * b)).clamp(-1.0, 1.0).acos().to_degrees())
}

fn run() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let (mut layers, mut projects) = (Vec::new(), Vec::<String>::new());
    while let Some(flag) = args.next() {
        if flag == "--team-maps" {
            game_data::init().map_err(|e| e.to_string())?;
            for map in maps::playable().filter(|m| m.mode != GameMode::FreeForAll) {
                if !projects.iter().any(|p| p == map.project) {
                    projects.push(map.project.to_owned());
                }
            }
            continue;
        }
        let value = args.next().ok_or_else(|| format!("{flag} needs a value"))?;
        match flag.as_str() {
            "--db" | "--patch" => layers.push(PathBuf::from(value)),
            "--project" => projects.push(value),
            _ => return Err(format!("unknown flag {flag}")),
        }
    }
    if layers.is_empty() || projects.is_empty() {
        return Err("--db and at least one --project (or --team-maps) are required".into());
    }
    let mut resources = Resources::open_layers(&layers).map_err(|e| e.to_string())?;
    let materials = Materials::load(&mut resources).map_err(|e| e.to_string())?;
    println!("project\tteam\tpoint\tx\ty\tz\tverdict\tharmful\tahead\toff_centre\tfacing");
    let (mut team_spawns, mut shut) = (0, 0);
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
            let (hit, faces) = facing(&map, feet, s.rotation);
            let ahead = hit.map_or(format!(">{SIGHT:.0}"), |t| format!("{t:.1}"));
            let centre = off_centre(&map, feet, s.rotation).map_or(String::new(), |a| format!("{a:.0}"));
            // Teams 0 and 1; a free for all's points (team 2) face anywhere.
            let faces = if (0..=1).contains(&s.team) {
                team_spawns += 1;
                match faces {
                    Facing::Clear => String::new(),
                    Facing::Wall(t) => {
                        shut += 1;
                        format!("faces a wall {t:.1} m ahead (under {FACING_CLEAR} m)")
                    }
                    Facing::Edge(t) => {
                        shut += 1;
                        format!("faces the map's edge {t:.1} m ahead (under {FACING_CLEAR} m)")
                    }
                }
            } else {
                String::new()
            };
            let p = s.position;
            println!(
                "{project}\t{}\t{}\t{:.3}\t{:.3}\t{:.3}\t{verdict}\t{harmful}\t{ahead}\t{centre}\t{faces}",
                s.team, s.point_id, p.x, p.y, p.z
            );
        }
    }
    eprintln!(
        "{} projects: {shut} of {team_spawns} team spawns face a wall or the map's edge within {FACING_CLEAR} m",
        projects.len()
    );
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
