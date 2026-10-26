//! The sight tree built by the surface area heuristic against the tree as it was (median splits, tight boxes) and against
//! testing every triangle, on the game's own maps: `visible` between points over the floors at eye height, and `ray_cast`
//! from them in random directions. The new tree must give what testing every triangle gives; where the two trees differ
//! every triangle decides, and each such query is printed. Also how long the queries take with each.
//!
//! ```text
//! SRP_MAPDATA_DIR=<mapdata-dump files> cargo test -p mapdata --release --test sight_trees -- --ignored --nocapture
//! ```

use std::time::Instant;

use mapdata::{Body, MapData, RayTree, Vec3, World};

#[test]
#[ignore = "needs SRP_MAPDATA_DIR"]
fn the_surface_area_tree_answers_as_the_old_tree_on_every_world() {
    let Some(dir) = std::env::var_os("SRP_MAPDATA_DIR").map(std::path::PathBuf::from) else {
        eprintln!("skipping: SRP_MAPDATA_DIR is not set");
        return;
    };
    let mut files: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "mapdata"))
        .collect();
    files.sort();
    assert!(!files.is_empty(), "no map files in {}", dir.display());
    let (mut queries, mut wrong_new, mut differ_old, mut hidden) = (0u64, 0u64, 0u64, 0u64);
    let (mut seconds_new, mut seconds_old) = (0f64, 0f64);
    let (mut build_new, mut build_old) = (0f64, 0f64);
    for path in &files {
        let map = MapData::read(path).unwrap();
        let triangles = map.bullet_triangles();
        let corners = || triangles.iter().map(|t| t.v);
        let started = Instant::now();
        let new = RayTree::from_corners(corners());
        build_new += started.elapsed().as_secs_f64();
        let started = Instant::now();
        let old = RayTree::from_corners_median(corners());
        build_old += started.elapsed().as_secs_f64();
        let world = World::build(map, mapdata::CELL, Body::default());
        let grid = &world.grid;
        let n = grid.len() as u64;
        let mut state = 0x9E37_79B9_7F4A_7C15u64 ^ n;
        let mut next = move || {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            state
        };
        let eye = |node: u64| grid.position(node as u32) + Vec3::new(0.0, 1.6, 0.0);
        // Pairs of eyes, nearby (a fight's range) and anywhere.
        let mut pairs = Vec::new();
        for k in 0..4000 {
            let a = next() % n;
            let b = if k % 2 == 0 { next() % n } else { (a + next() % 2000).min(n - 1) };
            pairs.push((eye(a), eye(b)));
        }
        let mut rays = Vec::new();
        for _ in 0..1000 {
            let a = eye(next() % n);
            let u = |x: u64| (x % 20_001) as f32 / 10_000.0 - 1.0;
            let d = Vec3::new(u(next()), u(next()) * 0.3, u(next()));
            if d.length() > 1e-3 {
                rays.push((a, d * (1.0 / d.length())));
            }
        }
        for (name, tree, total) in [("new", &new, &mut seconds_new), ("old", &old, &mut seconds_old)] {
            let started = Instant::now();
            let mut sink = 0u64;
            for &(a, b) in &pairs {
                sink += u64::from(tree.visible(a, b));
            }
            for &(a, d) in &rays {
                sink += u64::from(tree.ray_cast(a, d, 60.0).is_some());
            }
            *total += started.elapsed().as_secs_f64();
            std::hint::black_box((name, sink));
        }
        // The trees against each other first; where they differ, every triangle decides.
        for &(a, b) in &pairs {
            queries += 1;
            hidden += u64::from(!new.visible(a, b));
            if new.visible(a, b) != old.visible(a, b) {
                let want = new.visible_every_triangle(a, b);
                wrong_new += u64::from(new.visible(a, b) != want);
                differ_old += u64::from(old.visible(a, b) != want);
                eprintln!(
                    "DIFFERENT {}: visible from {a:?} to {b:?}: old {}, new {}, every triangle {want}",
                    world.map.project,
                    old.visible(a, b),
                    new.visible(a, b)
                );
            }
        }
        for &(a, d) in &rays {
            queries += 1;
            let (n, o) = (new.ray_cast(a, d, 60.0).map(f32::to_bits), old.ray_cast(a, d, 60.0).map(f32::to_bits));
            if n != o {
                let want = new.ray_cast_every_triangle(a, d, 60.0).map(f32::to_bits);
                wrong_new += u64::from(n != want);
                differ_old += u64::from(o != want);
                eprintln!(
                    "DIFFERENT {}: ray_cast from {a:?} along {d:?}: old {:?}, new {:?}, every triangle {:?}",
                    world.map.project,
                    o.map(f32::from_bits),
                    n.map(f32::from_bits),
                    want.map(f32::from_bits)
                );
            }
        }
        eprintln!("{}: {} triangles, {} nodes new / {} old", world.map.project, new.triangle_count(), new.memory_bytes(), old.memory_bytes());
    }
    eprintln!(
        "{} worlds, {queries} queries ({hidden} of the visible ones blocked): the new tree wrong {wrong_new} times, the old tree {differ_old}; \
         queries {seconds_new:.2} s new against {seconds_old:.2} s old; builds {build_new:.1} s new against {build_old:.1} s old",
        files.len()
    );
    assert_eq!(wrong_new, 0);
}
