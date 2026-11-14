//! The FFA spawn points (`docs/notes/ffa.md`): 0.31a2's mode-6 respawn
//! points checked against 0.26g's collision, one TSV row each on stdout
//! (`../checked.tsv`, which `../make_edits.py` reads).
//!
//! ```text
//! ffa-check-points --donor <0.31a2 resources.03.db> --db <0.26g resources.db> \
//!     --patch <resources.db.patch_12> ... --pair level_01/day_6=level_01/day_4 ...
//! ```
//!
//! A point stands where a player can: on the walker mesh (what the
//! character controller meets), checked in the 0.26g project the point
//! goes to:
//! - ground: a ray from 1.5 m above the point down to 0.5 m below it hits
//!   the walker mesh; the ground is that hit. Within 5 cm of the point it
//!   stays as it is (`kept`); else the point is moved onto it (`nudged`).
//!   Nothing hit, or a hit more than 0.5 m above the point (it is inside
//!   or under something), drops it;
//! - a standing body (radius 0.38 m, 1.8 m tall) is free there: nothing
//!   straight above for 1.8 m, nothing within its radius on rings of rays
//!   at 0.65 to 1.65 m (lower things are steps: the body climbs 0.6 m);
//! - a floor of the walk grid (cells of 0.25 m, eroded by the body's
//!   radius, only the floors reached from the project's own spawns and
//!   objectives) within 0.6 m and 0.35 m of height, where a body stands:
//!   no shut room, roof or ledge no one gets to.
//!
//! The project's own spawns go through the same check (stderr), as a
//! measure of how strict it is.

use std::path::PathBuf;
use std::process::ExitCode;

use binary_config_parser::ConfigValue;
use ffa_check_points::{Map, Verdict};
use mapdata::{Materials, Resources, Vec3};

/// A respawn point of the donor's project.
struct Donor {
    name: String,
    position: Vec3,
    rotation: Vec3,
    enemy_check_radius: f32,
    invulnerability_time: i32,
}

fn donor_points(resources: &mut Resources, project: &str) -> Result<Vec<Donor>, String> {
    let root = resources
        .config(&format!("projects/{project}/client_project"))
        .map_err(|e| e.to_string())?;
    let points = root
        .get("server_objects")
        .and_then(|objects| objects.get("respawn_points"))
        .map(ConfigValue::children)
        .unwrap_or_default();
    points
        .into_iter()
        .map(|point| {
            let error = |e: binary_config_parser::ConfigError| format!("{project}: {e}");
            let team = point.i32_at("team").map_err(error)?;
            if team != 2 {
                return Err(format!("{project}: a point of team {team}"));
            }
            Ok(Donor {
                name: point.str_at("name").map_err(error)?.to_owned(),
                position: Vec3::from_array(point.float3_at("position").map_err(error)?),
                rotation: Vec3::from_array(point.float3_at("rotation").map_err(error)?),
                enemy_check_radius: point.f32_at("enemy_check_radius").map_err(error)?,
                invulnerability_time: point.i32_at("invulnerability_time").map_err(error)?,
            })
        })
        .collect()
}

struct Cli {
    donor: PathBuf,
    layers: Vec<PathBuf>,
    /// (donor project, 0.26g project).
    pairs: Vec<(String, String)>,
}

fn parse() -> Result<Cli, String> {
    let mut args = std::env::args().skip(1);
    let (mut donor, mut db, mut patches, mut pairs) = (None, None, Vec::new(), Vec::new());
    while let Some(flag) = args.next() {
        let value = args.next().ok_or_else(|| format!("{flag} needs a value"))?;
        match flag.as_str() {
            "--donor" => donor = Some(PathBuf::from(value)),
            "--db" => db = Some(PathBuf::from(value)),
            "--patch" => patches.push(PathBuf::from(value)),
            "--pair" => {
                let (from, to) = value
                    .split_once('=')
                    .ok_or_else(|| format!("--pair {value}: <donor project>=<project>"))?;
                pairs.push((from.to_owned(), to.to_owned()));
            }
            _ => return Err(format!("unknown flag {flag}")),
        }
    }
    let mut layers = vec![db.ok_or("--db is required")?];
    layers.extend(patches);
    Ok(Cli {
        donor: donor.ok_or("--donor is required")?,
        layers,
        pairs,
    })
}

fn run() -> Result<(), String> {
    let cli = parse()?;
    let mut donor = Resources::open(&cli.donor).map_err(|e| e.to_string())?;
    let mut target = Resources::open_layers(&cli.layers).map_err(|e| e.to_string())?;
    let materials = Materials::load(&mut target).map_err(|e| e.to_string())?;
    println!(
        "project\tpoint\tx\ty\tz\trx\try\trz\tenemy_check_radius\tinvulnerability_time\tverdict\tground\treason"
    );
    for (from, to) in &cli.pairs {
        let points = donor_points(&mut donor, from)?;
        let map = Map::load(&mut target, &materials, to)?;
        // The project's own spawns: how many the check would move or drop.
        let mut own = [0; 3];
        for spawn in &map.data.spawns {
            let p = spawn.position;
            own[match map.check(p) {
                Verdict::Kept => 0,
                Verdict::Nudged { .. } => 1,
                Verdict::Dropped(_) => 2,
            }] += 1;
        }
        let mut counts = [0; 3];
        for point in &points {
            let p = point.position;
            let r = point.rotation;
            let (verdict, ground, reason) = match map.check(p) {
                Verdict::Kept => {
                    counts[0] += 1;
                    ("kept", String::new(), "on the ground".to_owned())
                }
                Verdict::Nudged { ground } => {
                    counts[1] += 1;
                    (
                        "nudged",
                        ground.to_string(),
                        format!("ground {:+.3} m", ground - p.y),
                    )
                }
                Verdict::Dropped(why) => {
                    counts[2] += 1;
                    ("dropped", String::new(), why)
                }
            };
            println!(
                "{to}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{verdict}\t{ground}\t{reason}",
                point.name,
                p.x,
                p.y,
                p.z,
                r.x,
                r.y,
                r.z,
                point.enemy_check_radius,
                point.invulnerability_time
            );
        }
        eprintln!(
            "{from} -> {to}: {} points, kept {}, nudged {}, dropped {}; \
             the project's own {} spawns would be kept {}, nudged {}, dropped {}",
            points.len(),
            counts[0],
            counts[1],
            counts[2],
            map.data.spawns.len(),
            own[0],
            own[1],
            own[2]
        );
    }
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        }
    }
}
