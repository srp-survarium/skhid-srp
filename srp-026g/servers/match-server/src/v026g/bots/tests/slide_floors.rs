//! [A316/A318] `grid.slide_floors` on the game's maps: what the pass reads and cuts on each map file, and the floors kept with and
//! without it. A probe (prints, asserts nothing); runs only when `SRP_MAPDATA_DIR` names the `mapdata-dump` files:
//!
//! ```text
//! SRP_MAPDATA_DIR=<dir of .mapdata files> cargo test -p match-server --release slide_floors_on_the_maps -- --ignored --nocapture
//! ```

use mapdata::{MapData, Vec3, WalkGrid};

use crate::v026g::bots::tuning::GridTuning;

#[test]
#[ignore = "a probe of the map files: prints the slide pass's numbers per map"]
fn slide_floors_on_the_maps() {
    let Some(dir) = std::env::var_os("SRP_MAPDATA_DIR") else {
        eprintln!("skipping: SRP_MAPDATA_DIR is not set");
        return;
    };
    let mut files: Vec<_> = std::fs::read_dir(&dir)
        .expect("the map dir")
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "mapdata"))
        .collect();
    files.sort();
    for path in files {
        let Ok(map) = MapData::read(&path) else {
            continue;
        };
        let walker = map.walker_triangles();
        let spawns: Vec<Vec3> = map.spawns.iter().map(|s| s.position).collect();
        let build = |slide_floors: bool| {
            let tuning = GridTuning { slide_floors, ..GridTuning::default() };
            WalkGrid::build_kept_with_links(
                &walker,
                map.play_area(mapdata::AREA_MARGIN),
                mapdata::CELL,
                tuning.body(),
                &map.grid_seeds(),
                &spawns,
                tuning.off_mesh_links(),
            )
        };
        let (off, _) = build(false);
        let (on, stats) = build(true);
        println!(
            "{}: floors kept off {} on {} ({:+}); slide pass {:?}",
            path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(),
            off.len(),
            on.len(),
            on.len() as i64 - off.len() as i64,
            stats.slide
        );
    }
}

/// The grids with and without the pass in one box of one map: `SRP_SLIDE_PROJECT` (a map file name), `SRP_SLIDE_BOX`
/// ("x0,z0,x1,z1,y0,y1"): the floors only one grid has, and for the floors both have, the links that differ.
#[test]
#[ignore = "a probe of one map's box: prints, asserts nothing"]
fn slide_floors_diff_in_a_box() {
    let (Some(dir), Ok(file), Ok(b)) = (std::env::var_os("SRP_MAPDATA_DIR"), std::env::var("SRP_SLIDE_PROJECT"), std::env::var("SRP_SLIDE_BOX")) else {
        eprintln!("skipping: SRP_MAPDATA_DIR / SRP_SLIDE_PROJECT / SRP_SLIDE_BOX not set");
        return;
    };
    let v: Vec<f32> = b.split(',').map(|x| x.trim().parse().unwrap()).collect();
    let map = MapData::read(&std::path::Path::new(&dir).join(&file)).expect("the map");
    let walker = map.walker_triangles();
    let spawns: Vec<Vec3> = map.spawns.iter().map(|s| s.position).collect();
    let build = |slide_floors: bool| {
        let tuning = GridTuning { slide_floors, ..GridTuning::default() };
        WalkGrid::build_kept_with_links(&walker, map.play_area(mapdata::AREA_MARGIN), mapdata::CELL, tuning.body(), &map.grid_seeds(), &spawns, tuning.off_mesh_links()).0
    };
    let (off, on) = (build(false), build(true));
    let inside = |p: Vec3| p.x >= v[0] && p.z >= v[1] && p.x <= v[2] && p.z <= v[3] && p.y >= v[4] && p.y <= v[5];
    let key = |p: Vec3| ((p.x * 100.0).round() as i32, (p.y * 100.0).round() as i32, (p.z * 100.0).round() as i32);
    let floors = |g: &WalkGrid| -> std::collections::BTreeMap<(i32, i32, i32), mapdata::NodeId> {
        g.floors_in([v[0], v[1], v[2], v[3]]).filter(|&n| inside(g.position(n))).map(|n| (key(g.position(n)), n)).collect()
    };
    let (fo, fn_) = (floors(&off), floors(&on));
    for (k, &n) in &fo {
        match fn_.get(k) {
            None => println!("only OFF: {:?}", off.position(n)),
            Some(&m) => {
                let lo: std::collections::BTreeSet<_> = off.links(n).map(|(t, kind)| (key(off.position(t)), format!("{kind:?}"))).collect();
                let ln: std::collections::BTreeSet<_> = on.links(m).map(|(t, kind)| (key(on.position(t)), format!("{kind:?}"))).collect();
                if lo != ln {
                    println!("links differ at {:?}: OFF only {:?}; ON only {:?}", off.position(n), lo.difference(&ln).collect::<Vec<_>>(), ln.difference(&lo).collect::<Vec<_>>());
                }
            }
        }
    }
    for (k, &m) in &fn_ {
        if !fo.contains_key(k) {
            println!("only ON: {:?}", on.position(m));
        }
    }
    println!("floors in the box: OFF {} ON {}", fo.len(), fn_.len());
}
