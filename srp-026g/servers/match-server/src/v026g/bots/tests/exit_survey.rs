//! The bases' exits on the real maps (`Tactics::exits`), for PR C's spawn exits: how many each side has, how far they lie
//! from its spawns, and how far apart. Read-only survey, only with `SRP_MAPDATA_DIR=<mapdata-dump files>`:
//!
//! ```text
//! SRP_MAPDATA_DIR=<dir> cargo test -p match-server --release exit_survey -- --ignored --nocapture
//! ```

use super::*;

#[test]
#[ignore]
pub(crate) fn exit_survey() {
    let Some(dir) = bench_settings().var_os("SRP_MAPDATA_DIR").map(std::path::PathBuf::from) else {
        eprintln!("skipping: SRP_MAPDATA_DIR is not set");
        return;
    };
    for level in 1..=8u8 {
        for mode in [2u8, 3, 4] {
            let project = format!("level_{level:02}/day_{mode}");
            let path = dir.join(mapdata::format::file_name(&project));
            let Ok(map) = mapdata::MapData::read(&path) else {
                continue;
            };
            let spawns: Vec<(i32, Vec3)> = map.spawns.iter().map(|s| (s.team, s.position)).collect();
            let world = Arc::new(World::build(map, mapdata::CELL, bench_tuning().grid_body()));
            let tactics = Tactics::new(world, 1.0);
            for side in [0, 1] {
                let ours: Vec<Vec3> = spawns.iter().filter(|(t, _)| *t == side).map(|(_, p)| *p).collect();
                let exits = tactics.exits(side);
                let from: Vec<String> = exits
                    .iter()
                    .map(|e| {
                        let near = ours.iter().map(|s| s.distance_xz(*e)).fold(f32::INFINITY, f32::min);
                        format!("({:.0}, {:.0}) {near:.0} m from the nearest spawn", e.x, e.z)
                    })
                    .collect();
                let apart = exits
                    .iter()
                    .enumerate()
                    .flat_map(|(i, a)| exits[i + 1..].iter().map(move |b| a.distance_xz(*b)))
                    .fold(f32::INFINITY, f32::min);
                eprintln!(
                    "exit survey {project} side {side}: {} spawns, {} exits{}: {}",
                    ours.len(),
                    exits.len(),
                    if exits.len() > 1 { format!(" (nearest two {apart:.0} m apart)") } else { String::new() },
                    from.join("; ")
                );
                // The spawn exits' ways out (PR C): toward the mode's objective spots and the enemy's spawn.
                let Ok(m) = crate::v026g::bots::objective::Mode::parse(mode) else {
                    continue;
                };
                let ways = tactics.ways_out(side, m.objective());
                let at: Vec<String> = ways
                    .iter()
                    .map(|w| {
                        let near = ours.iter().map(|s| s.distance_xz(*w)).fold(f32::INFINITY, f32::min);
                        format!("({:.0}, {:.0}) {near:.0} m", w.x, w.z)
                    })
                    .collect();
                eprintln!("ways out survey {project} side {side}: {} ways out: {}", ways.len(), at.join("; "));
            }
        }
    }
}
