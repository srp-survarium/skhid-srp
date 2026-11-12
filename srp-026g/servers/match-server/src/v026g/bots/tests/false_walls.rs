//! Which faces of the walker world the live players stand inside (A191): the live ground samples closer than 0.30 m to a wall section
//! of the capsule rule ([`super::standin_capsule`]), grouped by the collision library the face comes from and by whether the face is the
//! model's own `walker_collision` or the bullet `collision` that stands in for a `walker_collision` the archive does not have
//! (`MapData::triangles`: LIKELY, from bots caught against fruit trees' bullet meshes). A measurement: what the capsule is held by
//! that the client's capsule is not.
//!
//! `SRP_MAPDATA_DIR=<maps> SRP_HUMAN_DIR=<live samples> SRP_PROJECTS="level_01/day_2;.." cargo test -p match-server --release
//! false_walls -- --ignored --nocapture`

use std::collections::{HashMap, HashSet};

use super::standin_capsule::{Sections, nearest};

/// The samples nearer than this to a wall are the ones listed.
const CLOSE_M: f32 = 0.30;
/// The walker meshes' name part and the bullet collision's, as `MapData::place_faces` has them.
const WALKER_COLLISION: &str = ".model/walker_collision";
const BULLET_COLLISION: &str = ".model/collision";

/// The faces a walking body meets, with a tag `2 * library + stand_in` of the library they come from, and the libraries' names.
fn tagged_faces(map: &mapdata::MapData) -> Vec<(mapdata::Triangle, u32)> {
    let by_name: HashMap<&str, usize> = map.libraries.iter().enumerate().map(|(i, l)| (l.name.as_str(), i)).collect();
    let mut out = Vec::new();
    for placement in &map.placements {
        if !mapdata::project::CollisionFilter::WALKER.meets(placement.cgroup, placement.cmask) {
            continue;
        }
        let own = placement.library as usize;
        let library = &map.libraries[own];
        let stand_in = (library.faces.is_empty() && library.name.contains(WALKER_COLLISION))
            .then(|| library.name.replacen(WALKER_COLLISION, BULLET_COLLISION, 1))
            .and_then(|name| by_name.get(name.as_str()).copied());
        let used = stand_in.unwrap_or(own);
        let transform = placement.transform();
        let world: Vec<mapdata::Vec3> = map.libraries[used].vertices.iter().map(|&v| transform.apply(v)).collect();
        let tag = (used as u32) * 2 + u32::from(stand_in.is_some());
        for face in &map.libraries[used].faces {
            out.push((mapdata::Triangle { v: face.corners.map(|i| world[i as usize]), walker: face.surface.walker }, tag));
        }
    }
    out
}

#[test]
#[ignore = "a measurement; needs SRP_MAPDATA_DIR, SRP_HUMAN_DIR and SRP_PROJECTS"]
fn false_walls() {
    let (Some(maps), Ok(humans), Ok(projects)) =
        (std::env::var_os("SRP_MAPDATA_DIR"), std::env::var("SRP_HUMAN_DIR"), std::env::var("SRP_PROJECTS"))
    else {
        eprintln!("skipping: SRP_MAPDATA_DIR, SRP_HUMAN_DIR or SRP_PROJECTS is not set");
        return;
    };
    for project in projects.split(';').filter(|p| !p.trim().is_empty()) {
        let map = mapdata::MapData::read(&std::path::Path::new(&maps).join(mapdata::format::file_name(project))).unwrap();
        let sections = Sections::tagged(tagged_faces(&map), mapdata::Body::client().min_ground_normal_y);
        let path = std::path::Path::new(&humans).join(format!("{}.tsv", project.replace('/', "_")));
        let text = std::fs::read_to_string(path).unwrap_or_default();
        let (mut samples, mut close) = (0u32, 0u32);
        let mut per_tag: HashMap<u32, (u32, HashSet<String>)> = HashMap::new();
        let mut segs = Vec::new();
        for line in text.lines().skip(1) {
            let c: Vec<&str> = line.split('\t').collect();
            if c.len() < 10 || c[8] != "0" {
                continue;
            }
            let (Ok(x), Ok(y), Ok(z)) = (c[3].parse::<f32>(), c[4].parse::<f32>(), c[5].parse::<f32>()) else { continue };
            samples += 1;
            sections.near_tagged((x, z), y, c[6] == "1", CLOSE_M, &mut segs);
            let nearest_face = segs.iter().map(|(s, tag)| (nearest(s, (x, z)).1, *tag)).filter(|&(d, _)| d < CLOSE_M).min_by(|a, b| a.0.total_cmp(&b.0));
            if let Some((_, tag)) = nearest_face {
                close += 1;
                let entry = per_tag.entry(tag).or_default();
                entry.0 += 1;
                entry.1.insert(c[0].to_string());
            }
        }
        let stand_in: u32 = per_tag.iter().filter(|(t, _)| *t % 2 == 1).map(|(_, v)| v.0).sum();
        println!(
            "{project}: {samples} samples, {close} under {CLOSE_M} m of a wall ({:.2} %), of them {stand_in} at a face of a stand-in mesh (the bullet collision for a missing walker_collision)",
            100.0 * f64::from(close) / f64::from(samples.max(1))
        );
        let mut tags: Vec<_> = per_tag.into_iter().collect();
        tags.sort_by_key(|(_, v)| std::cmp::Reverse(v.0));
        for (tag, (n, matches)) in tags.into_iter().take(14) {
            let library = &map.libraries[(tag / 2) as usize];
            println!(
                "    {n:5} samples, {:2} matches: {}{} ({} faces, {} primitives)",
                matches.len(),
                library.name,
                if tag % 2 == 1 { "  [STAND-IN for a missing walker_collision]" } else { "" },
                library.faces.len(),
                library.primitives
            );
        }
    }
}
