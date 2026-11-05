//! The reach census: where live players stood that the bots' grid reaches
//! from no spawn, per map (the yardstick of the gap links,
//! `mapdata::grid::gap`; `reach-record.md`, 2026-10-26).
//!
//! ```text
//! SRP_MAPDATA_DIR=<mapdata-dump files> SRP_REACH_HUMANS=<dir of <project>.tsv> \
//!   cargo test -p mapdata --release --test reach_census -- --ignored --nocapture
//! ```
//!
//! The grids are built with the gap links ([`OffMeshLinks::Gaps`]);
//! `SRP_REACH_NO_GAPS` set, without (the switch's default).
//!
//! `<project>.tsv` (one per map variant, the mapdata file's stem): a header
//! line, then per sample `match place t_ms x feet z crouch sprint jumping
//! speed spawned`, tab-separated: the alive samples of the human places of
//! live matches with 12 or more humans (made by the reach record's
//! `extract_humans.py` from the worlds tables; no names). Samples in the
//! air (`jumping` 1) are left out. A sample stands on a floor when one is
//! within 0.75 m across and 0.45 m in height.
//!
//! Prints, per project and per map: the samples, those on a floor neither
//! team's spawns reach over the links (`unreached`), and those with no floor
//! near; then Chemical Plant's area C (x 38.4-55.9, z 63.9-69.4, feet
//! 8.5-11): its samples and whether each team reaches them.

use mapdata::{Body, MapData, NodeId, OffMeshLinks, Vec3, WalkGrid};

fn reached_from(grid: &WalkGrid, starts: &[NodeId]) -> Vec<bool> {
    let mut seen = vec![false; grid.len()];
    let mut stack: Vec<NodeId> = Vec::new();
    for &s in starts {
        if !seen[s as usize] {
            seen[s as usize] = true;
            stack.push(s);
        }
    }
    while let Some(f) = stack.pop() {
        for (m, kind) in grid.links(f) {
            if !seen[m as usize] && grid.link_cost(f, m, kind).is_finite() {
                seen[m as usize] = true;
                stack.push(m);
            }
        }
    }
    seen
}

fn floor_near(grid: &WalkGrid, p: Vec3) -> Option<NodeId> {
    let r = 0.75;
    grid.floors_in([p.x - r, p.z - r, p.x + r, p.z + r])
        .filter(|&f| {
            let q = grid.position(f);
            (q.y - p.y).abs() <= 0.45 && q.distance_xz(p) <= r
        })
        .min_by(|&a, &b| (grid.position(a) - p).length().total_cmp(&(grid.position(b) - p).length()))
}

#[test]
#[ignore = "needs SRP_MAPDATA_DIR and SRP_REACH_HUMANS"]
fn reach_census() {
    let (Some(maps), Some(humans)) = (std::env::var_os("SRP_MAPDATA_DIR"), std::env::var_os("SRP_REACH_HUMANS")) else {
        eprintln!("skipping: SRP_MAPDATA_DIR or SRP_REACH_HUMANS is not set");
        return;
    };
    let (maps, humans) = (std::path::PathBuf::from(maps), std::path::PathBuf::from(humans));
    let mut files: Vec<_> = std::fs::read_dir(&humans).unwrap().filter_map(|e| e.ok()).map(|e| e.path()).collect();
    files.sort();
    let mut by_map: std::collections::BTreeMap<String, [usize; 3]> = Default::default();
    let mut area_c = [0usize; 3];
    println!("project\tsamples\tunreached\tno_floor\toff_links");
    for file in files {
        let stem = file.file_stem().unwrap().to_string_lossy().to_string();
        let path = maps.join(format!("{stem}.mapdata"));
        if !path.exists() {
            continue;
        }
        let map = MapData::read(&path).expect("map");
        let spawns: Vec<Vec3> = map.spawns.iter().map(|s| s.position).collect();
        let links = if std::env::var_os("SRP_REACH_NO_GAPS").is_some() { OffMeshLinks::None } else { OffMeshLinks::Gaps };
        let (grid, _) = WalkGrid::build_kept_with_links(
            &map.walker_triangles(),
            map.play_area(mapdata::AREA_MARGIN),
            mapdata::CELL,
            Body::default(),
            &map.grid_seeds(),
            &spawns,
            links,
        );
        let teams: Vec<Vec<bool>> = (0..2)
            .map(|team| {
                let starts: Vec<NodeId> = map
                    .spawns
                    .iter()
                    .filter(|s| s.team == team)
                    .filter_map(|s| grid.locate(s.position, 3.0))
                    .collect();
                reached_from(&grid, &starts)
            })
            .collect();
        let mut row = [0usize; 3];
        for line in std::fs::read_to_string(&file).unwrap().lines().skip(1) {
            let v: Vec<&str> = line.split('\t').collect();
            if v.len() < 11 || v[8] != "0" {
                continue;
            }
            let num = |i: usize| v[i].parse::<f32>().unwrap_or(f32::NAN);
            let p = Vec3::new(num(3), num(4), num(5));
            row[0] += 1;
            match floor_near(&grid, p) {
                None => row[2] += 1,
                Some(f) => {
                    let (r0, r1) = (teams[0][f as usize], teams[1][f as usize]);
                    row[1] += usize::from(!r0 && !r1);
                    if stem.starts_with("level_04") && (38.4..=55.9).contains(&p.x) && (63.9..=69.4).contains(&p.z) && (8.5..=11.0).contains(&p.y) {
                        area_c[0] += 1;
                        area_c[1] += usize::from(r0);
                        area_c[2] += usize::from(r1);
                    }
                }
            }
        }
        println!("{stem}\t{}\t{}\t{}\t{}", row[0], row[1], row[2], grid.off_links().len());
        let e = by_map.entry(stem[..8].to_string()).or_default();
        for k in 0..3 {
            e[k] += row[k];
        }
    }
    for (map, [n, u, none]) in &by_map {
        println!("{map}: samples {n}, unreached {u} ({:.2} %), no floor {none}", 100.0 * *u as f64 / (*n).max(1) as f64);
    }
    println!("Chemical Plant area C: samples {}, reached by team 0 {}, by team 1 {}", area_c[0], area_c[1], area_c[2]);
}
