//! The bullet mesh's materials on the game's own maps. Runs only when `SRP_RESOURCES_DB` names the
//! 0.26g `resources.db` (skips with a note otherwise):
//!
//! ```text
//! SRP_RESOURCES_DB=<path to resources.db> cargo test -p mapdata --release --test pierce_maps -- --nocapture
//! ```

use std::collections::BTreeMap;

use mapdata::{FaceArmor, MapData, Materials, Pierce, Resources, Vec3};

fn load(project: &str) -> Option<MapData> {
    let Some(path) = std::env::var_os(mapdata::resources::PATH_VARIABLE) else {
        eprintln!("skipping: {} is not set", mapdata::resources::PATH_VARIABLE);
        return None;
    };
    let mut resources = Resources::open(path.as_ref()).ok()?;
    let materials = Materials::load(&mut resources).unwrap();
    Some(MapData::load(&mut resources, &materials, project).unwrap())
}

/// The projects of the owner's local matches: School, Vector Laboratory, Cologne, Rudnya, Tarakanovsky Fort.
const MAPS: [&str; 5] = ["level_02/day_2", "level_07/day_2", "level_08/day_2", "level_01/day_2", "level_05/day_2"];

/// The signed volume of each bullet library with enough triangles to be a body: the winding the
/// game's meshes use (`RayTree`'s front face rests on it).
#[test]
fn bullet_meshes_are_wound_with_the_normal_out() {
    let mut outward = 0usize;
    let mut inward = 0usize;
    for project in MAPS {
        let Some(map) = load(project) else { return };
        for library in &map.libraries {
            if library.faces.len() < 12 || !library.name.contains(".model/collision") {
                continue;
            }
            let volume: f64 = library
                .faces
                .iter()
                .map(|face| {
                    let [a, b, c] = face.corners.map(|i| library.vertices[i as usize]);
                    f64::from(a.dot(b.cross(c))) / 6.0
                })
                .sum();
            if volume > 1e-3 {
                outward += 1;
            } else if volume < -1e-3 {
                inward += 1;
            }
        }
    }
    eprintln!("signed volume > 0 (e1 x e2 out): {outward} libraries, < 0 (in): {inward}");
    assert!(outward > 5 * inward, "outward {outward}, inward {inward}");
}

#[test]
fn what_the_bullet_mesh_is_made_of() {
    for project in MAPS {
        let Some(map) = load(project) else { return };
        let mesh = map.bullet_mesh();
        let mut by_armor: BTreeMap<String, usize> = BTreeMap::new();
        for armor in mesh.armor() {
            let label = match armor {
                FaceArmor::Unknown => "unknown (no material)".to_owned(),
                FaceArmor::Material(armor) => format!("{:>6}", armor.value()),
            };
            *by_armor.entry(label).or_default() += 1;
        }
        eprintln!("{}: {} bullet triangles", map.project, mesh.len());
        for (label, n) in &by_armor {
            eprintln!("  armor {label:>22}: {n}");
        }
    }
}

/// How much of the bullet mesh is in bodies that are closed (every edge shared by two triangles): for those the
/// winding does not matter, for open sheets it decides which side a bullet is stopped from.
#[test]
fn how_much_of_the_bullet_mesh_is_closed() {
    use std::collections::HashMap;
    for project in MAPS {
        let Some(map) = load(project) else { return };
        let closed: Vec<bool> = map
            .libraries
            .iter()
            .map(|library| {
                // Corners at one place are one vertex (a hard edge has its corners twice).
                let mut ids: HashMap<[i64; 3], u32> = HashMap::new();
                let weld: Vec<u32> = library
                    .vertices
                    .iter()
                    .map(|v| {
                        let key = [v.x, v.y, v.z].map(|c| (f64::from(c) * 1e4).round() as i64);
                        let next = ids.len() as u32;
                        *ids.entry(key).or_insert(next)
                    })
                    .collect();
                let mut edges: HashMap<(u32, u32), i32> = HashMap::new();
                for face in &library.faces {
                    let t = face.corners;
                    for k in 0..3 {
                        let (a, b) = (weld[t[k] as usize], weld[t[(k + 1) % 3] as usize]);
                        *edges.entry((a.min(b), a.max(b))).or_default() += 1;
                    }
                }
                !library.faces.is_empty() && edges.values().all(|&n| n == 2)
            })
            .collect();
        let (mut in_closed, mut in_open) = (0usize, 0usize);
        for placement in &map.placements {
            if !mapdata::CollisionFilter::BULLET.meets(placement.cgroup, placement.cmask) {
                continue;
            }
            let library = &map.libraries[placement.library as usize];
            if closed[placement.library as usize] {
                in_closed += library.faces.len();
            } else {
                in_open += library.faces.len();
            }
        }
        eprintln!(
            "{project}: bullet triangles in closed bodies {in_closed}, in open meshes {in_open} ({:.0} %)",
            100.0 * in_open as f64 / (in_open + in_closed).max(1) as f64
        );
    }
}

/// What the new ray test costs against `visible`, on random eye-height lines of 5 to 40 m between the map's spawns
/// and points around them (numbers for the PR; asserts nothing).
#[test]
fn what_damage_share_costs_against_visible() {
    use std::time::Instant;
    for project in MAPS {
        let Some(map) = load(project) else { return };
        let tree = map.bullet_mesh().build_tree();
        let spawns: Vec<Vec3> = map.spawns.iter().map(|s| s.position + Vec3::new(0.0, 1.6, 0.0)).collect();
        let mut state = 0x9E37_79B9_7F4A_7C15u64;
        let mut next = move || {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            (state >> 40) as f32 / (1u64 << 24) as f32
        };
        let mut lines = Vec::new();
        for _ in 0..20_000 {
            let a = spawns[(next() * spawns.len() as f32) as usize % spawns.len()];
            let angle = next() * std::f32::consts::TAU;
            let length = 5.0 + next() * 35.0;
            let b = a + Vec3::new(angle.cos() * length, (next() - 0.5) * 3.0, angle.sin() * length);
            lines.push((a, b));
        }
        let pierce = Pierce::new(0.3125).unwrap();
        let t = Instant::now();
        let seen = lines.iter().filter(|&&(a, b)| tree.visible(a, b)).count();
        let visible_ns = t.elapsed().as_nanos() as f64 / lines.len() as f64;
        let t = Instant::now();
        let mut shares = [0usize; 3];
        for &(a, b) in &lines {
            let share = tree.damage_share(a, b, pierce).value();
            shares[if share >= 1.0 { 0 } else if share > 0.0 { 1 } else { 2 }] += 1;
        }
        let share_ns = t.elapsed().as_nanos() as f64 / lines.len() as f64;
        let t = Instant::now();
        let open = lines.iter().filter(|&&(a, b)| tree.reaches_over(a, b, pierce, 0.5)).count();
        let over_ns = t.elapsed().as_nanos() as f64 / lines.len() as f64;
        eprintln!(
            "{project}: visible {visible_ns:.0} ns ({seen} of {} clear), damage_share {share_ns:.0} ns, reaches_over 0.5 {over_ns:.0} ns ({open} open) (share 1: {}, between: {}, stopped: {})",
            lines.len(),
            shares[0],
            shares[1],
            shares[2]
        );
    }
}

/// What the live content layers do to the materials, and which faces of which maps have no material (so stop every
/// bullet as unknown). `SRP_PATCHES` = the resources.db patches in mount order, separated by `;`; `SRP_PROJECTS` =
/// extra projects (the ported maps), separated by `,`. Prints; asserts that no layer changes an armor the stock has.
#[test]
fn the_patch_layers_change_no_armor_and_the_unknown_faces_per_map() {
    use std::path::PathBuf;
    let Some(db) = std::env::var_os(mapdata::resources::PATH_VARIABLE) else { return };
    let patches: Vec<PathBuf> = std::env::var("SRP_PATCHES")
        .map(|list| list.split(';').filter(|p| !p.is_empty()).map(PathBuf::from).collect())
        .unwrap_or_default();
    let mut stock = Resources::open(db.as_ref()).unwrap();
    let stock_materials = Materials::load(&mut stock).unwrap();
    let mut layers = vec![PathBuf::from(&db)];
    layers.extend(patches.iter().cloned());
    let mut stacked = Resources::open_layers(&layers).unwrap();
    let materials = Materials::load(&mut stacked).unwrap();
    let mut changed = 0;
    for id in 0..400u16 {
        let (a, b) = (stock_materials.armor(Some(id)), materials.armor(Some(id)));
        if a != b {
            changed += 1;
            eprintln!("material {id}: stock {a:?}, with the patches {b:?}");
        }
    }
    eprintln!("{} patches over the stock db: {changed} armors differ", patches.len());
    assert_eq!(changed, 0);
    let mut projects: Vec<String> =
        survarium::v026g::maps::playable().map(|map| map.project.to_owned()).collect();
    if let Ok(extra) = std::env::var("SRP_PROJECTS") {
        projects.extend(extra.split(',').filter(|p| !p.is_empty()).map(str::to_owned));
    }
    projects.sort();
    projects.dedup();
    for project in &projects {
        let Ok(map) = MapData::load(&mut stacked, &materials, project) else {
            eprintln!("{project}: does not load");
            continue;
        };
        let census = map.bullet_mesh().build_tree().armor_census();
        eprintln!(
            "{project}: {} bullet faces, {} with armor, {} free, {} unknown ({:.2} %)",
            census.total,
            census.armored,
            census.free,
            census.unknown,
            100.0 * census.unknown as f64 / census.total.max(1) as f64
        );
    }
}

/// Why faces are unknown: the models with the most of them, and whether each has a `settings` file and `face_data`
/// (a face without either has no shading group to name its material). `SRP_PROJECT_OF_INTEREST` names the project.
#[test]
fn which_models_have_unknown_faces() {
    use std::collections::BTreeMap;
    let Some(db) = std::env::var_os(mapdata::resources::PATH_VARIABLE) else { return };
    let project = std::env::var("SRP_PROJECT_OF_INTEREST").unwrap_or_else(|_| "level_04/day_2".to_owned());
    let mut resources = Resources::open(db.as_ref()).unwrap();
    let materials = Materials::load(&mut resources).unwrap();
    let map = MapData::load(&mut resources, &materials, &project).unwrap();
    let mut by_model: BTreeMap<String, (usize, usize)> = BTreeMap::new();
    for placement in &map.placements {
        if !mapdata::CollisionFilter::BULLET.meets(placement.cgroup, placement.cmask) {
            continue;
        }
        let library = &map.libraries[placement.library as usize];
        let unknown = library.faces.iter().filter(|f| f.surface.armor == FaceArmor::Unknown).count();
        let entry = by_model.entry(library.name.clone()).or_default();
        entry.0 += unknown;
        entry.1 += library.faces.len();
    }
    let mut worst: Vec<_> = by_model.into_iter().filter(|(_, (u, _))| *u > 0).collect();
    worst.sort_by_key(|(_, (u, _))| std::cmp::Reverse(*u));
    for (name, (unknown, total)) in worst.iter().take(12) {
        let (path, _) = mapdata::library::split_scale(name);
        let model = path.split(".model").next().unwrap_or(path);
        eprintln!(
            "{project}: {unknown}/{total} unknown in {name}; settings {} face_data {} primitives {}",
            resources.exists(&format!("{model}.model/settings")),
            resources.exists(&format!("{path}/face_data")),
            resources.exists(&format!("{path}/exported_primitives")),
        );
    }
}
