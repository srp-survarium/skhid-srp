//! Checks battery support and pickup access against the mounted map's actual
//! collision. Reachability starts at both teams' spawns, not the objectives.
use std::path::PathBuf;

use clap::Parser;
use mapdata::math::Rotation;
use mapdata::project::{Shape, ShapeKind};
use mapdata::{Body, MapData, Materials, RayTree, Resources, Vec3, World};

#[derive(Parser)]
struct Cli {
    #[arg(long)]
    db: PathBuf,
    #[arg(long = "patch")]
    patches: Vec<PathBuf>,
    #[arg(long, default_value = "v011e_level_03/day_2")]
    project: String,
    /// Audit donor candidates before authoring the final layout.
    #[arg(long)]
    candidate_project: Option<String>,
    /// Also check that both teams can stand inside every Research capture zone.
    #[arg(long)]
    research_config: Option<String>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    let mut layers = vec![cli.db];
    layers.extend(cli.patches);
    let mut resources = Resources::open_layers(&layers)?;
    let materials = Materials::load(&mut resources)?;
    let map = MapData::load(&mut resources, &materials, &cli.project)?;
    let candidates = if let Some(project) = cli.candidate_project {
        MapData::load(&mut resources, &materials, &project)?.battery_spawners
    } else {
        map.battery_spawners.clone()
    };
    let walker = RayTree::build(&map.walker_triangles());
    // Fine grid: narrow paths and stairs must not disappear through sampling.
    let world = World::build(map, 0.125, Body::default());
    let grid = &world.grid;
    let reach: Vec<_> = [0, 1]
        .into_iter()
        .map(|team| {
            let seeds: Vec<_> = world
                .map
                .spawns
                .iter()
                .filter(|spawn| spawn.team == team)
                .filter_map(|spawn| grid.locate(spawn.position, 0.5))
                .collect();
            assert!(!seeds.is_empty(), "team {team} has no walkable spawn");
            grid.distances_from(&seeds)
        })
        .collect();
    let mut failed = 0;
    println!("index,x,y,z,support_gap,pickup_x,pickup_y,pickup_z,result");
    for (index, candidate) in candidates.iter().enumerate() {
        let item = candidate.item;
        let origin = item + Vec3::new(0.0, 0.3, 0.0);
        let down = Vec3::new(0.0, -1.0, 0.0);
        let floor = [
            walker.ray_cast(origin, down, 3.0),
            world.sight.ray_cast(origin, down, 3.0),
        ]
        .into_iter()
        .flatten()
        .min_by(f32::total_cmp)
        .map(|distance| distance - 0.3);
        // The battery origin is about 0.12 m above its base. Refuse buried
        // positions and unsupported floats; retain a small authoring tolerance.
        let supported = floor.is_some_and(|gap| (-0.03..=0.35).contains(&gap));
        let pickup = grid
            .floors_in([item.x - 2.0, item.z - 2.0, item.x + 2.0, item.z + 2.0])
            .filter(|&node| reach.iter().all(|team| team.reaches(node)))
            .map(|node| grid.position(node))
            .filter(|&point| {
                [1.0, 1.6].into_iter().any(|height| {
                    let eye = point + Vec3::new(0.0, height, 0.0);
                    (eye - item).length() <= 2.0 && world.sight.visible(eye, item)
                })
            })
            .min_by(|a, b| a.distance_xz(item).total_cmp(&b.distance_xz(item)));
        let status = match (supported, pickup.is_some()) {
            (true, true) => "PASS",
            (false, true) => "UNSUPPORTED",
            (true, false) => "NO_REACHABLE_PICKUP",
            (false, false) => "UNSUPPORTED_AND_UNREACHABLE",
        };
        if status != "PASS" {
            failed += 1;
        }
        let gap = floor.map_or_else(String::new, |v| format!("{v:.4}"));
        let point = pickup.map_or_else(
            || ",,".to_owned(),
            |p| format!("{:.4},{:.4},{:.4}", p.x, p.y, p.z),
        );
        println!(
            "{index},{:.4},{:.4},{:.4},{gap},{point},{status}",
            item.x, item.y, item.z
        );
    }
    eprintln!(
        "{}: {} candidates, {failed} failed support/pickup checks",
        cli.project,
        candidates.len()
    );
    if failed != 0 {
        return Err(format!("{failed} battery locations need review").into());
    }
    drop(world);
    if let Some(config) = cli.research_config {
        check_research(&mut resources, &materials, &config)?;
    }
    Ok(())
}

fn check_research(
    resources: &mut Resources,
    materials: &Materials,
    path: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let config = resources.config(path)?;
    let mode = config.get("mode").ok_or("missing Research mode")?;
    let project = mode
        .get("match_making_params")
        .ok_or("missing map parameters")?
        .str_at("map_name")?;
    let zones = mode
        .get("parameters")
        .and_then(|p| p.get("capture_zones"))
        .ok_or("missing capture zones")?;
    let world = World::build(
        MapData::load(resources, materials, project)?,
        0.125,
        Body::default(),
    );
    let grid = &world.grid;
    let reach: Vec<_> = [0, 1]
        .into_iter()
        .map(|team| {
            let seeds: Vec<_> = world
                .map
                .spawns
                .iter()
                .filter(|s| s.team == team)
                .filter_map(|s| grid.locate(s.position, 0.5))
                .collect();
            assert!(!seeds.is_empty());
            grid.distances_from(&seeds)
        })
        .collect();
    assert_eq!(zones.children().len(), 3);
    for zone in zones.children() {
        let id = zone.i32_at("cp_id")?;
        let mut accessible = false;
        for value in zone
            .get("shapes")
            .ok_or("missing capture shapes")?
            .children()
        {
            let shape = Shape {
                kind: match value.i32_at("type")? {
                    0 => ShapeKind::Sphere,
                    1 => ShapeKind::Box,
                    2 => ShapeKind::Cylinder,
                    _ => return Err("unknown capture shape".into()),
                },
                center: Vec3::from_array(value.float3_at("position")?),
                rotation: Rotation {
                    i: Vec3::from_array(value.float3_at("axis_x")?),
                    j: Vec3::from_array(value.float3_at("axis_y")?),
                    k: Vec3::from_array(value.float3_at("axis_z")?),
                },
                half: Vec3::from_array(value.float3_at("half")?),
            };
            let (lo, hi) = shape.bounds();
            accessible |= grid.floors_in([lo.x, lo.z, hi.x, hi.z]).any(|node| {
                reach.iter().all(|team| team.reaches(node))
                    && shape.contains(grid.position(node) + Vec3::new(0.0, 0.9, 0.0))
            });
        }
        if !accessible {
            return Err(format!(
                "Research station {id} is not capturable from both teams' reachable floors"
            )
            .into());
        }
        println!("Research station {id}: reachable capture position from both teams");
    }
    Ok(())
}
